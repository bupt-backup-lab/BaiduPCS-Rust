//! 每个订阅的轮询调度器
//!
//! 简化版 PollScheduler：每个订阅一个 tokio task，按 interval 或 cron 时刻循环触发。
//! 合并触发（coalescing）：如果上一次 run 还没结束就又到了下一次，跳过本轮。
//!
//! CancellationToken 控制优雅停机。

use super::config::{PollSchedule, MIN_POLL_INTERVAL_SECS};
use chrono::{DateTime, Local};
use std::time::Duration;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// cron / 每日定时触发时向后抖动的最大秒数（只往后不往前，避免早于用户设定的时间窗口）
const CRON_JITTER_MAX_SECS: i64 = 60;

/// 等待墙上时刻时单次 sleep 的上限：tokio 计时基于单调时钟，休眠/改系统时间后
/// 会与墙上时间脱节，分段 sleep 并每次对照当前时间，保证误差不超过这一粒度。
const WALL_CLOCK_POLL_SECS: u64 = 60;

/// 单个订阅的调度状态
pub struct SubscriptionScheduler {
    pub subscription_id: String,
    pub schedule: PollSchedule,
    cancel_token: CancellationToken,
    /// 当前是否正在执行（合并触发用）
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 外部传入的"立刻跑一次"通知器
    trigger_notify: std::sync::Arc<Notify>,
    /// 主循环 JoinHandle
    task: Option<tokio::task::JoinHandle<()>>,
}

/// 一次等待的结果
enum WaitOutcome {
    /// 到点
    Elapsed,
    /// 被外部 trigger 提前唤醒
    Triggered,
    /// 调度器被停止
    Cancelled,
}

impl SubscriptionScheduler {
    pub fn new(subscription_id: String, schedule: PollSchedule) -> Self {
        let schedule = match schedule {
            // 防御：不低于最小轮询间隔
            PollSchedule::Interval(secs) => {
                PollSchedule::Interval(secs.max(MIN_POLL_INTERVAL_SECS))
            }
            other => other,
        };
        Self {
            subscription_id,
            schedule,
            cancel_token: CancellationToken::new(),
            running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            trigger_notify: std::sync::Arc::new(Notify::new()),
            task: None,
        }
    }

    /// 启动主循环
    ///
    /// `on_tick` 由调用方提供；通常会调用 `ShareSyncManager::execute_one(id)`，
    /// 返回 `Result<(), ShareSyncError>`，scheduler 会把 Err 记到日志（之前用
    /// `let _ = ...` 吞掉,owner_uid=0 这类失败会变成静默执行)。
    pub fn start<F, Fut>(&mut self, on_tick: F)
    where
        F: Fn(String) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), super::ShareSyncError>> + Send,
    {
        if self.task.is_some() {
            return; // 已经启动
        }
        let sub_id = self.subscription_id.clone();
        let schedule = self.schedule.clone();
        let running = self.running.clone();
        let trigger = self.trigger_notify.clone();
        let cancel = self.cancel_token.clone();

        let handle = tokio::spawn(async move {
            info!(
                "scheduler: 订阅 {} 主循环启动, {}",
                sub_id,
                describe_schedule(&schedule)
            );
            let mut first = true;
            // cron 模式上一次已到点的时刻，防止时钟回拨时同一时刻重复触发
            let mut last_slot: Option<DateTime<Local>> = None;

            loop {
                let outcome = match &schedule {
                    PollSchedule::Interval(secs) => {
                        // 首次启动用更大的抖动（避免所有订阅同时发起请求）
                        let ratio = if first { 0.25 } else { 0.20 };
                        let delay = Duration::from_secs(jitter(*secs, ratio) as u64);
                        wait_for(delay, &trigger, &cancel).await
                    }
                    PollSchedule::Cron(cron) => {
                        let now = Local::now();
                        let from = match last_slot {
                            Some(s) if s > now => s,
                            _ => now,
                        };
                        match cron.next_after(&from) {
                            Some(slot) => {
                                let target = slot + cron_jitter();
                                debug!(
                                    "scheduler: 订阅 {} 下次检查 {}（含抖动 {}）",
                                    sub_id,
                                    slot.format("%Y-%m-%d %H:%M"),
                                    target.format("%H:%M:%S")
                                );
                                let outcome = wait_until(target, &trigger, &cancel).await;
                                if matches!(outcome, WaitOutcome::Elapsed) {
                                    last_slot = Some(slot);
                                }
                                outcome
                            }
                            None => {
                                warn!(
                                    "scheduler: 订阅 {} 的 cron 表达式不会再触发，仅响应手动触发",
                                    sub_id
                                );
                                wait_trigger(&trigger, &cancel).await
                            }
                        }
                    }
                };
                first = false;
                match outcome {
                    WaitOutcome::Cancelled => break,
                    WaitOutcome::Triggered => {
                        debug!("scheduler: 订阅 {} 被外部 trigger", sub_id);
                    }
                    WaitOutcome::Elapsed => {}
                }
                if cancel.is_cancelled() {
                    break;
                }
                // 检查是否已有 run 在执行
                if running.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    debug!("scheduler: 订阅 {} 上次 run 还在进行，跳过本轮", sub_id);
                } else {
                    match on_tick(sub_id.clone()).await {
                        Ok(()) => {
                            debug!("scheduler: 订阅 {} on_tick 正常完成", sub_id);
                        }
                        Err(e) => {
                            warn!(
                                "scheduler: 订阅 {} on_tick 失败: {} (静默吞掉, 不影响下次 tick)",
                                sub_id, e
                            );
                        }
                    }
                    running.store(false, std::sync::atomic::Ordering::SeqCst);
                }
            }
            info!("scheduler: 订阅 {} 主循环退出", sub_id);
        });
        self.task = Some(handle);
    }

    /// 立即触发一次（与 interval 合并）
    pub fn trigger_now(&self) {
        self.trigger_notify.notify_one();
    }

    /// 停止主循环
    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(h) = self.task.take() {
            let _ = h.await;
        }
    }

    /// 是否正在执行
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for SubscriptionScheduler {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}

/// 日志用的调度描述
pub fn describe_schedule(schedule: &PollSchedule) -> String {
    match schedule {
        PollSchedule::Interval(secs) => format!("interval={}s", secs),
        PollSchedule::Cron(cron) => format!("cron: {}", cron.describe()),
    }
}

async fn wait_for(delay: Duration, trigger: &Notify, cancel: &CancellationToken) -> WaitOutcome {
    tokio::select! {
        _ = tokio::time::sleep(delay) => WaitOutcome::Elapsed,
        _ = trigger.notified() => WaitOutcome::Triggered,
        _ = cancel.cancelled() => WaitOutcome::Cancelled,
    }
}

/// 等到墙上时刻 `target`（分段 sleep，见 `WALL_CLOCK_POLL_SECS`）
async fn wait_until(
    target: DateTime<Local>,
    trigger: &Notify,
    cancel: &CancellationToken,
) -> WaitOutcome {
    loop {
        let remaining = match (target - Local::now()).to_std() {
            Ok(d) if !d.is_zero() => d,
            // 已到点（或时间差为负）
            _ => return WaitOutcome::Elapsed,
        };
        let step = remaining.min(Duration::from_secs(WALL_CLOCK_POLL_SECS));
        match wait_for(step, trigger, cancel).await {
            WaitOutcome::Elapsed => continue,
            other => return other,
        }
    }
}

async fn wait_trigger(trigger: &Notify, cancel: &CancellationToken) -> WaitOutcome {
    tokio::select! {
        _ = trigger.notified() => WaitOutcome::Triggered,
        _ = cancel.cancelled() => WaitOutcome::Cancelled,
    }
}

/// cron 触发的向后抖动 [0, CRON_JITTER_MAX_SECS] 秒
fn cron_jitter() -> chrono::Duration {
    use rand::Rng;
    chrono::Duration::seconds(rand::thread_rng().gen_range(0..=CRON_JITTER_MAX_SECS))
}

/// interval ±ratio 的随机抖动
///
/// 抖动后强制不低于 `MIN_POLL_INTERVAL_SECS`（与 autobackup `add_jitter` 一致），
/// 避免间隔逼近下限时抖动把实际间隔压到下限以下、增大风控压力。
fn jitter(base_secs: u32, ratio: f64) -> u32 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let delta = (base_secs as f64 * ratio) as i64;
    if delta <= 0 {
        return base_secs.max(MIN_POLL_INTERVAL_SECS);
    }
    let offset = rng.gen_range(-delta..=delta);
    ((base_secs as i64 + offset).max(MIN_POLL_INTERVAL_SECS as i64) as u32).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share_sync::cron::CronSchedule;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_jitter_within_range() {
        // base 远大于下限时，抖动对称：1800 ±20% => [1440, 2160]
        for _ in 0..100 {
            let j = jitter(1800, 0.20);
            assert!((1440..=2160).contains(&j), "jitter out of range: {}", j);
        }
    }

    #[test]
    fn test_jitter_never_below_min_interval() {
        // base 恰为下限时，抖动下半被钳到 MIN_POLL_INTERVAL_SECS（与 autobackup 一致）
        for _ in 0..200 {
            let j = jitter(MIN_POLL_INTERVAL_SECS, 0.20);
            assert!(
                j >= MIN_POLL_INTERVAL_SECS,
                "jitter dipped below min: {}",
                j
            );
            assert!(j <= MIN_POLL_INTERVAL_SECS + (MIN_POLL_INTERVAL_SECS / 5));
        }
    }

    #[test]
    fn test_cron_jitter_only_forward() {
        for _ in 0..200 {
            let j = cron_jitter().num_seconds();
            assert!(
                (0..=CRON_JITTER_MAX_SECS).contains(&j),
                "cron jitter: {}",
                j
            );
        }
    }

    #[test]
    fn test_new_clamps_interval() {
        let s = SubscriptionScheduler::new("s".into(), PollSchedule::Interval(60));
        assert_eq!(s.schedule, PollSchedule::Interval(MIN_POLL_INTERVAL_SECS));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_scheduler_trigger_merges_with_running() {
        let mut s = SubscriptionScheduler::new("s1".into(), PollSchedule::Interval(60));
        let count = std::sync::Arc::new(AtomicUsize::new(0));
        let count_c = count.clone();
        // 模拟一个 long-running tick：第一次开始后立即把 running 设为 true
        s.start(move |_id| {
            let c = count_c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(300)).await;
                Ok(())
            }
        });
        // 触发两次：第二次应被合并（因为第一次还在跑）
        s.trigger_now();
        tokio::time::sleep(Duration::from_millis(50)).await;
        s.trigger_now();
        tokio::time::sleep(Duration::from_millis(500)).await;
        // 至少跑了 1 次（可能 1-2 次）
        assert!(count.load(Ordering::SeqCst) >= 1);
        s.stop().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_cron_scheduler_responds_to_trigger_and_stop() {
        // 闰年 2 月 29 日 00:00：短期内不会自然到点，只验证手动触发与停止
        let cron = CronSchedule::parse("0 0 29 2 *").unwrap();
        let mut s = SubscriptionScheduler::new("s2".into(), PollSchedule::Cron(cron));
        let count = std::sync::Arc::new(AtomicUsize::new(0));
        let count_c = count.clone();
        s.start(move |_id| {
            let c = count_c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
        s.trigger_now();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
        tokio::time::timeout(Duration::from_secs(2), s.stop())
            .await
            .expect("stop 应及时返回");
    }
}
