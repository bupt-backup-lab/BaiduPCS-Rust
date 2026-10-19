//! 分享同步的 cron 轮询表达式（issue #179）
//!
//! 只支持标准 5 字段 crontab：`分 时 日 月 星期`，语义对齐 Vixie cron：
//! - 每个字段支持 `*`、`n`、`a-b`、`*/s`、`a-b/s`、`n/s`（= `n-最大值/s`）以及逗号列表；
//! - 月份可写 `JAN`-`DEC`，星期可写 `SUN`-`SAT`（大小写不敏感），星期 `0` 与 `7` 都表示周日；
//! - 「日」与「星期」都被限定（都不以 `*` 开头）时，二者满足其一即触发；否则二者需同时满足；
//! - 支持 `@hourly` / `@daily` / `@midnight` / `@weekly` / `@monthly` / `@yearly` / `@annually`。
//!
//! 时间按服务器本地时区计算（与 autobackup「每日定时」一致）。不支持秒字段与 `L`/`W`/`#` 等扩展。

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike};

/// 向后搜索下一次触发时间的最大天数：覆盖闰年 2 月 29 日（最长 8 年一遇，如 2096→2104）
const MAX_SEARCH_DAYS: i64 = 366 * 8 + 2;

/// 最小间隔校验的模拟窗口：跨过一整年即可覆盖月/年边界上的相邻触发
const GAP_CHECK_DAYS: i64 = 400;
/// 最小间隔校验最多模拟的触发次数（防止稠密表达式拖慢校验）
const GAP_CHECK_MAX_FIRES: usize = 20_000;

const WEEKDAY_NAMES: [&str; 7] = ["日", "一", "二", "三", "四", "五", "六"];

/// 解析后的 cron 表达式
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSchedule {
    minutes: u64,
    hours: u64,
    days_of_month: u64,
    months: u64,
    days_of_week: u64,
    /// 「日」字段是否以 `*` 开头（Vixie 语义：决定日/星期是 AND 还是 OR）
    dom_star: bool,
    /// 「星期」字段是否以 `*` 开头
    dow_star: bool,
}

#[derive(Clone, Copy)]
struct FieldSpec {
    name: &'static str,
    min: u32,
    max: u32,
    names: &'static [&'static str],
}

const MINUTE: FieldSpec = FieldSpec {
    name: "分钟",
    min: 0,
    max: 59,
    names: &[],
};
const HOUR: FieldSpec = FieldSpec {
    name: "小时",
    min: 0,
    max: 23,
    names: &[],
};
const DAY_OF_MONTH: FieldSpec = FieldSpec {
    name: "日",
    min: 1,
    max: 31,
    names: &[],
};
const MONTH: FieldSpec = FieldSpec {
    name: "月",
    min: 1,
    max: 12,
    names: &[
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ],
};
// 星期允许写到 7（周日），解析后再折叠到 0
const DAY_OF_WEEK: FieldSpec = FieldSpec {
    name: "星期",
    min: 0,
    max: 7,
    names: &["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"],
};

impl CronSchedule {
    /// 解析 5 字段 cron 表达式
    pub fn parse(expr: &str) -> Result<Self, String> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Err("cron 表达式不能为空".into());
        }
        let expanded = match expr.to_ascii_lowercase().as_str() {
            "@hourly" => "0 * * * *",
            "@daily" | "@midnight" => "0 0 * * *",
            "@weekly" => "0 0 * * 0",
            "@monthly" => "0 0 1 * *",
            "@yearly" | "@annually" => "0 0 1 1 *",
            s if s.starts_with('@') => return Err(format!("不支持的 cron 宏: {}", expr)),
            _ => expr,
        };
        let fields: Vec<&str> = expanded.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(format!(
                "cron 表达式需要 5 个字段（分 时 日 月 星期），当前为 {} 个",
                fields.len()
            ));
        }
        let minutes = parse_field(fields[0], MINUTE)?;
        let hours = parse_field(fields[1], HOUR)?;
        let days_of_month = parse_field(fields[2], DAY_OF_MONTH)?;
        let months = parse_field(fields[3], MONTH)?;
        let mut days_of_week = parse_field(fields[4], DAY_OF_WEEK)?;
        if days_of_week & (1 << 7) != 0 {
            days_of_week = (days_of_week & !(1 << 7)) | 1;
        }
        Ok(Self {
            minutes,
            hours,
            days_of_month,
            months,
            days_of_week,
            dom_star: fields[2].starts_with('*'),
            dow_star: fields[4].starts_with('*'),
        })
    }

    /// 每天固定时刻（「每日定时」模式复用 cron 调度）
    pub fn daily(hour: u32, minute: u32) -> Self {
        Self {
            minutes: 1 << minute.min(59),
            hours: 1 << hour.min(23),
            days_of_month: full_mask(DAY_OF_MONTH),
            months: full_mask(MONTH),
            days_of_week: full_mask_range(0, 6),
            dom_star: true,
            dow_star: true,
        }
    }

    fn day_matches(&self, date: NaiveDate) -> bool {
        if !has(self.months, date.month()) {
            return false;
        }
        let dom_ok = has(self.days_of_month, date.day());
        let dow_ok = has(self.days_of_week, date.weekday().num_days_from_sunday());
        if self.dom_star || self.dow_star {
            dom_ok && dow_ok
        } else {
            dom_ok || dow_ok
        }
    }

    /// 严格晚于 `after` 的下一个触发时刻（墙上时间，分钟精度）
    fn next_naive_after(&self, after: NaiveDateTime) -> Option<NaiveDateTime> {
        let start = after.with_second(0).and_then(|t| t.with_nanosecond(0))? + Duration::minutes(1);
        let start_date = start.date();
        for offset in 0..MAX_SEARCH_DAYS {
            let date = start_date + Duration::days(offset);
            if !self.day_matches(date) {
                continue;
            }
            let first_day = offset == 0;
            for hour in bits(self.hours) {
                if first_day && hour < start.hour() {
                    continue;
                }
                for minute in bits(self.minutes) {
                    if first_day && hour == start.hour() && minute < start.minute() {
                        continue;
                    }
                    return date.and_hms_opt(hour, minute, 0);
                }
            }
        }
        None
    }

    /// 严格晚于 `after` 的下一个触发时刻（按 `after` 所在时区的墙上时间匹配）。
    ///
    /// 夏令时跳过的不存在时刻会被略过；回拨导致的重复时刻取较早的一个。
    pub fn next_after<Tz: TimeZone>(&self, after: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        let tz = after.timezone();
        let mut cursor = after.naive_local();
        for _ in 0..1000 {
            let naive = self.next_naive_after(cursor)?;
            if let Some(t) = tz.from_local_datetime(&naive).earliest() {
                if t > *after {
                    return Some(t);
                }
            }
            cursor = naive;
        }
        None
    }

    /// 从 `from` 起的接下来 `count` 次触发时刻
    pub fn upcoming<Tz: TimeZone>(&self, from: &DateTime<Tz>, count: usize) -> Vec<DateTime<Tz>> {
        let mut out = Vec::with_capacity(count);
        let mut cursor = from.clone();
        while out.len() < count {
            match self.next_after(&cursor) {
                Some(t) => {
                    cursor = t.clone();
                    out.push(t);
                }
                None => break,
            }
        }
        out
    }

    /// 校验：表达式必须能触发，且任意相邻两次触发的间隔不小于 `min_gap_secs`。
    ///
    /// 在不受时区影响的墙上时间轴上模拟约一年的触发序列（年/月边界上的相邻触发也会被覆盖）。
    pub fn check_min_gap(&self, min_gap_secs: u32) -> Result<(), String> {
        let origin = NaiveDate::from_ymd_opt(2000, 1, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .expect("valid origin");
        let mut prev = match self.next_naive_after(origin - Duration::minutes(1)) {
            Some(t) => t,
            None => return Err("该 cron 表达式永远不会触发（日期不存在）".into()),
        };
        let horizon = origin + Duration::days(GAP_CHECK_DAYS);
        let mut fires = 1usize;
        while prev < horizon && fires < GAP_CHECK_MAX_FIRES {
            let next = match self.next_naive_after(prev) {
                Some(t) => t,
                None => break,
            };
            let gap = (next - prev).num_seconds();
            if gap < min_gap_secs as i64 {
                return Err(format!(
                    "触发过于频繁：{} 与 {} 仅相隔 {} 分钟，两次检查之间至少需要 {} 分钟",
                    prev.format("%H:%M"),
                    next.format("%H:%M"),
                    gap / 60,
                    min_gap_secs / 60
                ));
            }
            prev = next;
            fires += 1;
        }
        Ok(())
    }

    /// 中文可读描述，如 `*/10 11-13 * * 6` → 「每周六 11:00-13:59 期间，每 10 分钟一次」
    pub fn describe(&self) -> String {
        let day = self.describe_days();
        let time = self.describe_time();
        format!("{} {}", day, time)
    }

    fn describe_days(&self) -> String {
        let mut dom_full = self.days_of_month == full_mask(DAY_OF_MONTH);
        let mut dow_full = self.days_of_week == full_mask_range(0, 6);
        // 日/星期都被限定时取并集：其中一个覆盖全部取值即等于每天
        if !self.dom_star && !self.dow_star && (dom_full || dow_full) {
            dom_full = true;
            dow_full = true;
        }
        let dom_desc = || describe_set(self.days_of_month, 1, 31, "日", "天");
        let dow_desc = || describe_weekdays(self.days_of_week);

        let month_full = self.months == full_mask(MONTH);
        let month_prefix = if month_full {
            String::new()
        } else {
            describe_set(self.months, 1, 12, "月", "个月")
        };

        let day_part = match (dom_full, dow_full) {
            (true, true) => {
                return if month_full {
                    "每天".into()
                } else {
                    format!("{}的每天", month_prefix)
                };
            }
            (true, false) => format!("每周{}", dow_desc()),
            (false, true) => {
                if month_full {
                    format!("每月{}", dom_desc())
                } else {
                    dom_desc()
                }
            }
            (false, false) => {
                let joiner = if self.dom_star || self.dow_star {
                    "且为"
                } else {
                    "或"
                };
                let dom = if month_full {
                    format!("每月{}", dom_desc())
                } else {
                    dom_desc()
                };
                format!("{}{}每周{}", dom, joiner, dow_desc())
            }
        };
        if month_full {
            day_part
        } else {
            format!("{}的{}", month_prefix, day_part)
        }
    }

    fn describe_time(&self) -> String {
        let minute_list: Vec<u32> = bits(self.minutes).collect();
        let hour_list: Vec<u32> = bits(self.hours).collect();
        let hours_full = self.hours == full_mask(HOUR);

        if minute_list.len() == 1 {
            let m = minute_list[0];
            if hours_full {
                return format!("每小时的第 {} 分钟", m);
            }
            if let Some((start, step)) = step_pattern(self.hours, 0, 23, 3) {
                return format!("{:02}:{:02} 起每 {} 小时一次", start, m, step);
            }
            if hour_list.len() <= 8 {
                return hour_list
                    .iter()
                    .map(|h| format!("{:02}:{:02}", h, m))
                    .collect::<Vec<_>>()
                    .join("、");
            }
            return format!(
                "{}，每小时的第 {} 分钟",
                describe_hour_window(self.hours),
                m
            );
        }

        let freq = if self.minutes == full_mask(MINUTE) {
            "每分钟一次".to_string()
        } else if let Some((start, step)) = step_pattern(self.minutes, 0, 59, 2) {
            if start == 0 {
                format!("每 {} 分钟一次", step)
            } else {
                format!("从第 {} 分钟起每 {} 分钟一次", start, step)
            }
        } else {
            format!(
                "每小时的第 {} 分钟",
                describe_set(self.minutes, 0, 59, "", "分钟")
            )
        };
        if hours_full {
            format!("全天{}", freq)
        } else {
            format!("{}，{}", describe_hour_window(self.hours), freq)
        }
    }
}

fn describe_hour_window(hours: u64) -> String {
    if let Some((start, step)) = step_pattern(hours, 0, 23, 3) {
        return format!("{} 点起每隔 {} 小时的那一小时内", start, step);
    }
    let ranges = ranges_of(hours);
    let text = ranges
        .iter()
        .map(|&(a, b)| format!("{:02}:00-{:02}:59", a, b))
        .collect::<Vec<_>>()
        .join("、");
    format!("{} 期间", text)
}

fn describe_weekdays(mask: u64) -> String {
    let ranges = ranges_of(mask);
    ranges
        .iter()
        .map(|&(a, b)| {
            if a == b {
                WEEKDAY_NAMES[a as usize].to_string()
            } else if b == a + 1 {
                format!(
                    "{}、{}",
                    WEEKDAY_NAMES[a as usize], WEEKDAY_NAMES[b as usize]
                )
            } else {
                format!(
                    "{}至周{}",
                    WEEKDAY_NAMES[a as usize], WEEKDAY_NAMES[b as usize]
                )
            }
        })
        .collect::<Vec<_>>()
        .join("、")
}

/// 把数值集合描述为「1-5、10、15{suffix}」或「每隔 2 {step_unit}」之类
fn describe_set(mask: u64, min: u32, max: u32, suffix: &str, step_unit: &str) -> String {
    if let Some((start, step)) = step_pattern(mask, min, max, 3) {
        return if start == min {
            format!("每隔 {} {}", step, step_unit)
        } else {
            format!("从 {}{} 起每隔 {} {}", start, suffix, step, step_unit)
        };
    }
    let text = ranges_of(mask)
        .iter()
        .map(|&(a, b)| {
            if a == b {
                a.to_string()
            } else {
                format!("{}-{}", a, b)
            }
        })
        .collect::<Vec<_>>()
        .join("、");
    format!("{}{}", text, suffix)
}

/// 识别「从 start 开始每 step 一次直到上限」的等差集合（step > 1，至少 `min_len` 个值）
fn step_pattern(mask: u64, min: u32, max: u32, min_len: usize) -> Option<(u32, u32)> {
    let values: Vec<u32> = bits(mask).collect();
    if values.len() < min_len.max(2) {
        return None;
    }
    let step = values[1] - values[0];
    if step <= 1 {
        return None;
    }
    let arithmetic = values.windows(2).all(|w| w[1] - w[0] == step);
    let reaches_end = values[values.len() - 1] + step > max;
    let starts_early = values[0] < min + step;
    if arithmetic && reaches_end && starts_early {
        Some((values[0], step))
    } else {
        None
    }
}

fn ranges_of(mask: u64) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for v in bits(mask) {
        match out.last_mut() {
            Some((_, end)) if *end + 1 == v => *end = v,
            _ => out.push((v, v)),
        }
    }
    out
}

fn bits(mask: u64) -> impl Iterator<Item = u32> {
    (0..64u32).filter(move |i| mask & (1u64 << i) != 0)
}

fn has(mask: u64, v: u32) -> bool {
    v < 64 && mask & (1u64 << v) != 0
}

fn full_mask(spec: FieldSpec) -> u64 {
    full_mask_range(spec.min, spec.max)
}

fn full_mask_range(min: u32, max: u32) -> u64 {
    (min..=max).fold(0u64, |acc, v| acc | (1u64 << v))
}

fn parse_field(raw: &str, spec: FieldSpec) -> Result<u64, String> {
    let mut mask = 0u64;
    for part in raw.split(',') {
        if part.is_empty() {
            return Err(format!("{}字段存在空的列表项: {}", spec.name, raw));
        }
        let (range_part, step) = match part.split_once('/') {
            Some((r, s)) => {
                let step: u32 = s
                    .parse()
                    .map_err(|_| format!("{}字段步长无效: {}", spec.name, part))?;
                if step == 0 {
                    return Err(format!("{}字段步长不能为 0: {}", spec.name, part));
                }
                (r, Some(step))
            }
            None => (part, None),
        };
        let (lo, hi) = if range_part == "*" {
            (spec.min, spec.max)
        } else if let Some((a, b)) = range_part.split_once('-') {
            let lo = parse_value(a, spec)?;
            let hi = parse_value(b, spec)?;
            if lo > hi {
                return Err(format!("{}字段范围起点大于终点: {}", spec.name, part));
            }
            (lo, hi)
        } else {
            let v = parse_value(range_part, spec)?;
            // `n/s` 按 Vixie 语义展开为 `n-最大值/s`
            if step.is_some() {
                (v, spec.max)
            } else {
                (v, v)
            }
        };
        let step = step.unwrap_or(1);
        let mut v = lo;
        while v <= hi {
            mask |= 1u64 << v;
            v += step;
        }
    }
    Ok(mask)
}

fn parse_value(raw: &str, spec: FieldSpec) -> Result<u32, String> {
    let upper = raw.to_ascii_uppercase();
    if let Some(idx) = spec.names.iter().position(|n| *n == upper) {
        // 月份名从 1 开始，星期名从 0（SUN）开始
        return Ok(idx as u32 + if spec.min == 1 { 1 } else { 0 });
    }
    let v: u32 = raw
        .parse()
        .map_err(|_| format!("{}字段取值无效: {}", spec.name, raw))?;
    if v < spec.min || v > spec.max {
        return Err(format!(
            "{}字段取值 {} 超出范围 {}-{}",
            spec.name, v, spec.min, spec.max
        ));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    fn fmt(ts: &[DateTime<Utc>]) -> Vec<String> {
        ts.iter()
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .collect()
    }

    #[test]
    fn issue_179_example_every_10_min_saturday_window() {
        let c = CronSchedule::parse("*/10 11-13 * * 6").unwrap();
        // 2026-10-17 是周六；从周六 13:45 开始
        let next = c.upcoming(&at(2026, 10, 17, 13, 45), 3);
        assert_eq!(
            fmt(&next),
            vec!["2026-10-17 13:50", "2026-10-24 11:00", "2026-10-24 11:10"]
        );
        assert_eq!(c.describe(), "每周六 11:00-13:59 期间，每 10 分钟一次");
        assert!(c.check_min_gap(600).is_ok());
    }

    #[test]
    fn next_after_is_strictly_later() {
        let c = CronSchedule::parse("0 3 * * *").unwrap();
        let t = at(2026, 1, 1, 3, 0);
        assert_eq!(c.next_after(&t).unwrap(), at(2026, 1, 2, 3, 0));
        let t = at(2026, 1, 1, 2, 59) + Duration::seconds(30);
        assert_eq!(c.next_after(&t).unwrap(), at(2026, 1, 1, 3, 0));
    }

    #[test]
    fn daily_matches_equivalent_expression() {
        assert_eq!(
            CronSchedule::daily(3, 30),
            CronSchedule::parse("30 3 * * *").unwrap()
        );
        assert_eq!(CronSchedule::daily(3, 30).describe(), "每天 03:30");
    }

    #[test]
    fn dom_and_dow_both_restricted_use_or() {
        // 每月 1 日 或 每周一
        let c = CronSchedule::parse("0 9 1 * 1").unwrap();
        // 2026-06-01 是周一，06-08 周一，07-01 周三
        let next = c.upcoming(&at(2026, 6, 25, 0, 0), 3);
        assert_eq!(
            fmt(&next),
            vec!["2026-06-29 09:00", "2026-07-01 09:00", "2026-07-06 09:00"]
        );
    }

    #[test]
    fn starred_dom_with_step_uses_and() {
        // dom 以 * 开头 → AND：奇数日且为周一
        let c = CronSchedule::parse("0 9 */2 * 1").unwrap();
        let next = c.upcoming(&at(2026, 6, 1, 10, 0), 2);
        // 6 月周一: 1,8,15,22,29 → 奇数日: 15, 29
        assert_eq!(fmt(&next), vec!["2026-06-15 09:00", "2026-06-29 09:00"]);
    }

    #[test]
    fn sunday_as_7_and_names() {
        let a = CronSchedule::parse("0 8 * * 7").unwrap();
        let b = CronSchedule::parse("0 8 * * sun").unwrap();
        let c = CronSchedule::parse("0 8 * * 0").unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
        let m = CronSchedule::parse("0 0 1 jan-mar *").unwrap();
        assert_eq!(m, CronSchedule::parse("0 0 1 1-3 *").unwrap());
    }

    #[test]
    fn leap_day_is_found() {
        let c = CronSchedule::parse("0 0 29 2 *").unwrap();
        assert_eq!(
            c.next_after(&at(2026, 3, 1, 0, 0)).unwrap(),
            at(2028, 2, 29, 0, 0)
        );
        assert!(c.check_min_gap(600).is_ok());
    }

    #[test]
    fn impossible_date_never_fires() {
        let c = CronSchedule::parse("0 0 30 2 *").unwrap();
        assert!(c.next_after(&at(2026, 1, 1, 0, 0)).is_none());
        assert!(c.check_min_gap(600).unwrap_err().contains("永远不会触发"));
    }

    #[test]
    fn min_gap_rejects_dense_expressions() {
        assert!(CronSchedule::parse("* * * * *")
            .unwrap()
            .check_min_gap(600)
            .is_err());
        assert!(CronSchedule::parse("*/5 * * * *")
            .unwrap()
            .check_min_gap(600)
            .is_err());
        assert!(CronSchedule::parse("0,55 * * * *")
            .unwrap()
            .check_min_gap(600)
            .is_err());
        // 跨天边界：23:55 → 次日 00:00
        assert!(CronSchedule::parse("0,55 0,23 * * *")
            .unwrap()
            .check_min_gap(600)
            .is_err());
        assert!(CronSchedule::parse("*/10 * * * *")
            .unwrap()
            .check_min_gap(600)
            .is_ok());
        assert!(CronSchedule::parse("@hourly")
            .unwrap()
            .check_min_gap(600)
            .is_ok());
    }

    #[test]
    fn parse_errors() {
        for bad in [
            "",
            "* * * *",
            "* * * * * *",
            "60 * * * *",
            "* 24 * * *",
            "* * 0 * *",
            "* * * 13 *",
            "* * * * 8",
            "*/0 * * * *",
            "5-1 * * * *",
            "a * * * *",
            "1,,2 * * * *",
            "@reboot",
        ] {
            assert!(CronSchedule::parse(bad).is_err(), "应拒绝: {:?}", bad);
        }
    }

    #[test]
    fn value_with_step_expands_to_max() {
        let a = CronSchedule::parse("5/20 * * * *").unwrap();
        let b = CronSchedule::parse("5,25,45 * * * *").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn respects_timezone_wall_clock() {
        let tz = FixedOffset::east_opt(8 * 3600).unwrap();
        let c = CronSchedule::parse("0 11 * * *").unwrap();
        let from = tz.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
        let next = c.next_after(&from).unwrap();
        assert_eq!(next, tz.with_ymd_and_hms(2026, 10, 19, 11, 0, 0).unwrap());
    }

    #[test]
    fn descriptions() {
        let cases = [
            ("30 8,12,18 * * 1-5", "每周一至周五 08:30、12:30、18:30"),
            ("0 */2 * * *", "每天 00:00 起每 2 小时一次"),
            ("15 * * * *", "每天 每小时的第 15 分钟"),
            (
                "*/30 20-23 * * 5,6",
                "每周五、六 20:00-23:59 期间，每 30 分钟一次",
            ),
            ("0 9 1,15 * *", "每月1、15日 09:00"),
            ("0 9 1 * 1", "每月1日或每周一 09:00"),
            ("0 0 1 1 *", "1月的1日 00:00"),
            (
                "0,30 9-11,14-17 * * *",
                "每天 09:00-11:59、14:00-17:59 期间，每 30 分钟一次",
            ),
            ("0,20 * * * *", "每天 全天每小时的第 0、20 分钟"),
            ("0 9 1-31 * 1", "每天 09:00"),
            ("0 0 */2 * *", "每月每隔 2 天 00:00"),
            ("0 0 1 */3 *", "每隔 3 个月的1日 00:00"),
            ("0 9-18 * * *", "每天 09:00-18:59 期间，每小时的第 0 分钟"),
        ];
        for (expr, want) in cases {
            assert_eq!(
                CronSchedule::parse(expr).unwrap().describe(),
                want,
                "{}",
                expr
            );
        }
    }
}
