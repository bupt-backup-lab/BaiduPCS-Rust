//! 迁移前完整备份
//!
//! 在**任何破坏性迁移操作之前**（`session.json → session.json.migrated` 改名、
//! `accounts.json` 写入、SQLite `ALTER` / 历史回填、`.meta` 回写、
//! `autobackup_configs.json` 回写）把关键数据完整复制到
//! `config/backups/pre_migration_<时间戳>/`，使得万一迁移失败用户能整目录还原。
//!
//! 设计约束：
//!   - 备份**内容必须完整**：覆盖下方 [`PRE_MIGRATION_FILES`] 清单 + 真实 DB 路径
//!     （含 `-wal` / `-shm` 边车文件）+ 整个 `wal_dir` 目录。
//!   - DB 路径**按 `AppConfig.persistence.db_path` 取真实值**，不写死。
//!   - 备份**失败仅告警、不阻塞启动**（不抛错），避免把可恢复的小问题升级成启动失败；
//!     真正的破坏性写入若随后失败，会另行进入只读保护模式。
//!   - **每个程序版本备份一次**：以 `config/backups/.last_backup_version` 记录已为哪个
//!     版本做过迁移前备份，版本变化（升级 / 降级）即重新备份；新备份成功后只保留最近
//!     [`PRE_MIGRATION_KEEP`] 份，避免无限堆积。

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// 迁移前需要备份的 `config/` 下单文件清单（相对 `config_dir`，不存在则跳过）。
pub const PRE_MIGRATION_FILES: &[&str] = &[
    "app.toml",
    "session.json",
    "session.json.migrated",
    "accounts.json",
    "accounts.json.bak",
    "autobackup_configs.json",
];

/// 备份完成标记文件名。**仅当备份成功（实际复制了内容）后**才写入；
/// 据此判断一个 `pre_migration_*` 目录是否为**有效**（完整）备份。
const BACKUP_MANIFEST: &str = "backup_manifest.json";

/// 版本标记文件名（位于 `config/backups/` 下），内容为最近一次**成功**完成迁移前
/// 备份（或确认为全新安装）时的程序版本号。
const VERSION_MARKER: &str = ".last_backup_version";

/// 迁移前备份目录名前缀。
const BACKUP_DIR_PREFIX: &str = "pre_migration_";

/// 备份目录名中的时间戳格式（定长，字典序即时间序）。
const BACKUP_TS_FORMAT: &str = "%Y%m%d_%H%M%S";

/// 新备份成功后保留的有效迁移前备份份数（含刚创建的这份）。
const PRE_MIGRATION_KEEP: usize = 3;

/// 是否需要创建迁移前备份。
///
/// 采用「**当前程序版本是否已为本数据目录备份过**」的判据，而不是去枚举具体哪一步
/// 迁移（schema / `.meta` / 历史回填）待执行 —— 后者容易漏判（例如 `accounts.json`
/// 与 `backup_configs.owner_uid` 都已就绪，但 `.meta` 或历史表仍需回填时，窄判据会
/// 跳过备份后又写盘）。
///
/// 返回 `true` 当且仅当：
///   1. 存在升级前的既有数据（`db_path` / `session.json` / `accounts.json`
///      任一存在）—— 全新安装无需备份；**且**
///   2. 版本标记（[`VERSION_MARKER`]）缺失或与 `app_version` 不一致 —— 即本版本
///      尚未成功备份过（缺失对应「从引入版本标记之前的版本升级上来」）。
///
/// 这样可保证：每次升级（版本变化）在**任何破坏性迁移发生前**恰好备份一次；同一
/// 版本的后续启动因标记一致而跳过。备份失败时不写标记，下次启动重做，避免「以为有
/// 备份其实是空壳」。
///
/// 取舍：**只看版本标记，不校验备份目录本身是否仍有效存在**。同版本内用户手动删除
/// / 损坏了备份，也不会重新备份，直到下次版本变化。原因：
///   - 同版本后续启动时迁移早已执行完，此时补做的备份是**迁移后**的数据，名不副实；
///   - 「标记一致但无有效备份」也是**全新安装**的常态（无数据时只写标记不备份），
///     若据此告警会让每个全新安装每次启动都误报。
pub fn needs_pre_migration_backup(
    config_dir: &Path,
    db_path: &Path,
    session_path: &Path,
    accounts_path: &Path,
    app_version: &str,
) -> bool {
    if !(db_path.exists() || session_path.exists() || accounts_path.exists()) {
        return false;
    }
    read_backup_version(config_dir).as_deref() != Some(app_version)
}

/// 读取版本标记；不存在 / 读取失败返回 `None`。
fn read_backup_version(config_dir: &Path) -> Option<String> {
    std::fs::read_to_string(config_dir.join("backups").join(VERSION_MARKER))
        .ok()
        .map(|s| s.trim().to_string())
}

/// 确保版本标记记录为 `app_version`（已一致则不写盘）。
///
/// 用于**无需备份**的启动（全新安装 / 本版本已备份）：全新安装此时写入标记，避免
/// 首次启动产生数据后，第二次启动被误判为「升级」而多做一次备份。
pub async fn ensure_backup_version_recorded(config_dir: &Path, app_version: &str) {
    if read_backup_version(config_dir).as_deref() == Some(app_version) {
        return;
    }
    if let Err(e) = write_backup_version(config_dir, app_version).await {
        warn!("写入迁移前备份版本标记失败（不阻塞启动）: {e:#}");
    }
}

async fn write_backup_version(config_dir: &Path, app_version: &str) -> Result<()> {
    let backups = config_dir.join("backups");
    tokio::fs::create_dir_all(&backups)
        .await
        .with_context(|| format!("创建备份目录失败: {}", backups.display()))?;
    let marker = backups.join(VERSION_MARKER);
    tokio::fs::write(&marker, app_version)
        .await
        .with_context(|| format!("写入版本标记失败: {}", marker.display()))
}

/// 若 `name` 严格形如 `pre_migration_<YYYYmmdd_HHMMSS>`，返回其时间戳部分。
///
/// 额外要求定长 15 字符：chrono 的 `%m` / `%d` 等也接受 1 位数字，不定长的名字会
/// 破坏「字典序即时间序」，此类（多为手动改名的）目录一律视为无法识别、不参与清理。
fn backup_dir_timestamp(name: &str) -> Option<&str> {
    let ts = name.strip_prefix(BACKUP_DIR_PREFIX)?;
    if ts.len() != 15 {
        return None;
    }
    chrono::NaiveDateTime::parse_from_str(ts, BACKUP_TS_FORMAT)
        .ok()
        .map(|_| ts)
}

/// 单个备份目录是否为**有效**（完整）备份：存在 [`BACKUP_MANIFEST`] 完成标记，
/// **且** manifest 声明的关键文件都确实存在（见 [`manifest_required_satisfied`]）。
fn is_valid_backup_dir(dir: &Path) -> bool {
    dir.join(BACKUP_MANIFEST).exists() && manifest_required_satisfied(dir)
}

/// 探测 `config/backups/` 下是否已存在**有效**的 `pre_migration_*` 备份目录。
#[cfg(test)]
fn valid_pre_migration_backup_exists(config_dir: &Path) -> bool {
    let backups = config_dir.join("backups");
    match std::fs::read_dir(&backups) {
        Ok(rd) => rd.filter_map(|e| e.ok()).any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(BACKUP_DIR_PREFIX)
                && is_valid_backup_dir(&e.path())
        }),
        Err(_) => false,
    }
}

/// 清理旧的迁移前备份：保留 `current`（刚创建的那份）以及除它之外最新的
/// `keep - 1` 份**有效**备份，删除其余更早的有效备份与残缺（无效）备份目录。
///
/// - 只处理名称严格为 `pre_migration_<时间戳>` 的**真实目录**（不跟随符号链接），
///   无法识别的目录（如用户手动改名）一律不动。
/// - `current` 显式保留，即使系统时钟回拨导致它的时间戳不是最新的也不会被删。
/// - 仅应在**新备份成功之后**调用：同一版本只会成功备份一次，因而迁移失败后的
///   反复重启不会把升级前那份好的备份挤掉。
/// - 删除失败仅告警。返回已删除的目录列表。
async fn prune_pre_migration_backups(
    config_dir: &Path,
    current: &Path,
    keep: usize,
) -> Vec<PathBuf> {
    let backups = config_dir.join("backups");
    let rd = match std::fs::read_dir(&backups) {
        Ok(rd) => rd,
        Err(e) => {
            warn!("清理旧迁移前备份：读取 {} 失败（{}）", backups.display(), e);
            return Vec::new();
        }
    };
    let current_name = current.file_name().map(|n| n.to_owned());
    // (时间戳, 路径)；不含 current
    let mut candidates: Vec<(String, PathBuf)> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .filter(|e| Some(e.file_name()) != current_name)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            backup_dir_timestamp(&name).map(|ts| (ts.to_string(), e.path()))
        })
        .collect();
    // 时间戳新 → 旧
    candidates.sort_by(|a, b| b.0.cmp(&a.0));

    let mut kept = 1usize; // current
    let mut removed = Vec::new();
    for (_, dir) in candidates {
        if kept < keep && is_valid_backup_dir(&dir) {
            kept += 1;
            continue;
        }
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => {
                info!("已清理旧迁移前备份: {}", dir.display());
                removed.push(dir);
            }
            Err(e) => warn!("清理旧迁移前备份 {} 失败（{}）", dir.display(), e),
        }
    }
    removed
}

/// 校验备份目录的 manifest 是否表明这是一份有效（完整）备份。
///
/// 语义（manifest 作为完成标记应当可靠，故对损坏/半写入从严）：
/// - manifest **读取失败 / JSON 解析失败** → `false`：损坏或半写入的标记不算有效备份。
/// - manifest 存在但**不含 `required` 字段**（如**旧版**备份）→ `true`：按「存在即
///   有效」的旧口径处理，**向后兼容**、不让既有备份失效。
/// - 含 `required` 数组 → 逐项确认对应文件/目录在备份目录内存在；任一缺失 → `false`。
/// - 另对 wal 目录做内容齐全校验：优先按 `wal_files`（相对路径清单）逐项核对，旧
///   manifest 仅有 `wal_file_count` 时回退到数量校验，两者皆无则跳过。
fn manifest_required_satisfied(backup_dir: &Path) -> bool {
    let raw = match std::fs::read_to_string(backup_dir.join(BACKUP_MANIFEST)) {
        Ok(s) => s,
        Err(_) => return false, // 读不出 manifest（损坏/权限）：不算有效
    };
    let json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return false, // JSON 解析失败（半写入/损坏）：不算有效
    };
    // 1) required 列出的关键文件 / 目录必须都存在
    let required_ok = match json.get("required").and_then(|v| v.as_array()) {
        None => true, // 旧 manifest 无 required：向后兼容
        Some(items) => items
            .iter()
            .filter_map(|v| v.as_str())
            .all(|name| backup_dir.join(name).exists()),
    };
    if !required_ok {
        return false;
    }

    // 2) wal 目录内容齐全校验。优先用 `wal_files` 逐项核对（既证明数量、也证明每个
    //    具体文件都在，可识别「缺 a.meta、多 b.meta、数量却不变」）；旧 manifest 无
    //    `wal_files` 时回退到 `wal_file_count` 数量校验（#10）；两者都无则跳过。
    let wal_name = json.get("wal_backup_dir").and_then(|v| v.as_str());
    if let Some(name) = wal_name {
        let wal_root = backup_dir.join(name);
        match json.get("wal_files").and_then(|v| v.as_array()) {
            Some(files) => {
                let all_present = files
                    .iter()
                    .filter_map(|v| v.as_str())
                    .all(|rel| wal_root.join(rel).is_file());
                if !all_present {
                    return false;
                }
            }
            None => {
                if let Some(expect) = json.get("wal_file_count").and_then(|v| v.as_u64()) {
                    if (count_files_recursive(&wal_root) as u64) < expect {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// 递归统计目录下的普通文件数（出错按 0 计）。用于兼容旧 manifest 的 `wal_file_count`
/// 数量校验。
fn count_files_recursive(dir: &Path) -> usize {
    let mut count = 0usize;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let rd = match std::fs::read_dir(&cur) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in rd.filter_map(|e| e.ok()) {
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => stack.push(entry.path()),
                Ok(_) => count += 1,
                Err(_) => {}
            }
        }
    }
    count
}

/// 一次迁移前备份的结果。
#[derive(Debug, Clone)]
pub struct PreMigrationBackup {
    /// 备份目录（`config/backups/pre_migration_<时间戳>/`）。
    pub dir: PathBuf,
    /// 已成功复制的条目（文件 / 目录）描述列表。
    pub copied: Vec<String>,
}

/// 在任何破坏性迁移前创建一份完整备份。
///
/// * `config_dir` —— 配置目录（通常为 `config`）。
/// * `db_path` —— **真实** SQLite 路径（来自 `AppConfig.persistence.db_path`）。
/// * `wal_dir` —— WAL / `.meta` 目录。
/// * `app_version` —— 当前程序版本，写入 manifest 与版本标记。
///
/// 单文件 / 子目录复制失败仅 `warn!` 跳过（best-effort）；但若**一项都没复制成功**
/// 或**完成标记（manifest）写入失败**，则返回 `Err`（不写 manifest、不写版本标记，
/// 目录不算有效备份，下次启动重做）。调用方对 `Err` 仅告警、不阻塞启动。
///
/// 成功后写入版本标记，并清理旧备份只保留最近 [`PRE_MIGRATION_KEEP`] 份；这两步
/// 失败仅告警，不影响本次备份的有效性。
pub async fn create_pre_migration_backup(
    config_dir: &Path,
    db_path: &Path,
    wal_dir: &Path,
    app_version: &str,
) -> Result<PreMigrationBackup> {
    let ts = chrono::Local::now().format(BACKUP_TS_FORMAT).to_string();
    let backup_dir = config_dir
        .join("backups")
        .join(format!("{BACKUP_DIR_PREFIX}{ts}"));
    tokio::fs::create_dir_all(&backup_dir)
        .await
        .with_context(|| format!("创建迁移前备份目录失败: {}", backup_dir.display()))?;

    let mut copied: Vec<String> = Vec::new();
    // `required`：本次按既有数据**应当**出现在备份目录里的关键文件（以在备份目录中
    // 的相对名记录）。启动时据此严格校验有效性——这些文件若因复制失败缺失，则该备份
    // 视为无效并重做。只把**源端确实存在**的关键文件计入，避免要求本就不存在的文件。
    let mut required: Vec<String> = Vec::new();

    // 1) config/ 下单文件清单
    for name in PRE_MIGRATION_FILES {
        let src = config_dir.join(name);
        copy_file_best_effort(&src, &backup_dir.join(name), &mut copied).await;
    }
    // session.json / accounts.json 属关键数据，源端存在则计入 required
    for key in ["session.json", "accounts.json"] {
        if config_dir.join(key).exists() {
            required.push(key.to_string());
        }
    }

    // 2) 真实 DB 路径 + WAL/SHM 边车文件
    if let Some(db_name) = db_path.file_name() {
        copy_file_best_effort(db_path, &backup_dir.join(db_name), &mut copied).await;
        // 真实 DB 文件若源端存在则属关键文件，计入 required
        if db_path.exists() {
            required.push(db_name.to_string_lossy().into_owned());
        }
        // `-wal`（可能含尚未 checkpoint 的数据）/ `-shm` 边车：源端存在则也计入 required
        for ext in ["-wal", "-shm"] {
            let side = sidecar_path(db_path, ext);
            if let Some(side_name) = side.file_name() {
                copy_file_best_effort(&side, &backup_dir.join(side_name), &mut copied).await;
                if side.exists() {
                    required.push(side_name.to_string_lossy().into_owned());
                }
            }
        }
    } else {
        warn!(
            "迁移前备份：db_path 无文件名，跳过 DB 复制: {}",
            db_path.display()
        );
    }

    // 3) 整个 wal_dir 目录（任务 `.meta` 状态）：源端存在则计入 required。
    //    内容**必须完整复制**——若有任一文件复制 / 读取 / 遍历失败，则该目录备份不
    //    可信，视为整体备份失败（不写 manifest、返回 Err），下次启动重做；仅校验目录
    //    存在不足以保证内容齐全。完整复制时把每个文件的相对路径记入 `wal_files`，启
    //    动时据此**逐项**核对备份目录内对应文件是否存在，防止备份后内容被删 / 替换。
    let mut wal_files: Option<Vec<String>> = None;
    let mut wal_backup_name: Option<String> = None;
    if wal_dir.exists() {
        let dest_name = wal_dir
            .file_name()
            .map(|n| n.to_owned())
            .unwrap_or_else(|| std::ffi::OsString::from("wal"));
        let dest = backup_dir.join(&dest_name);
        match copy_dir_recursive(wal_dir, &dest).await {
            Ok(stats) if stats.failed == 0 => {
                copied.push(format!(
                    "{} (目录, {} 个文件)",
                    dest.display(),
                    stats.copied()
                ));
                let name = dest_name.to_string_lossy().into_owned();
                required.push(name.clone());
                wal_backup_name = Some(name);
                wal_files = Some(stats.files);
            }
            Ok(stats) => {
                anyhow::bail!(
                    "迁移前备份：wal 目录 {} 复制不完整（成功 {} / 失败 {}），不写入完成标记",
                    wal_dir.display(),
                    stats.copied(),
                    stats.failed
                );
            }
            Err(e) => anyhow::bail!(
                "迁移前备份：复制 wal 目录 {} 失败（{}）",
                wal_dir.display(),
                e
            ),
        }
    }

    // 复制结果为空（既有数据应被复制却一项都没成功）→ 视为**失败**：不写
    // manifest，使该目录不被认作有效备份，下次启动会重做。调用方据 Err 告警。
    if copied.is_empty() {
        anyhow::bail!(
            "迁移前备份未复制任何内容（目标 {}），不写入完成标记",
            backup_dir.display()
        );
    }

    // 写入完成标记（manifest）。**必须是最后一步**——只有它存在才算有效备份。
    let manifest = serde_json::json!({
        "completed_at": chrono::Local::now().to_rfc3339(),
        // 做这份备份的程序版本（即本次升级到的版本），便于还原时辨认是哪次升级前的数据
        "app_version": app_version,
        "db_path": db_path.display().to_string(),
        "wal_dir": wal_dir.display().to_string(),
        "required": required,
        // 备份目录内 wal 子目录名与其下文件相对路径清单；启动校验时据此逐项核对内容齐全。
        "wal_backup_dir": wal_backup_name,
        "wal_files": wal_files,
        "copied": copied,
    });
    let manifest_str = serde_json::to_string_pretty(&manifest)
        .unwrap_or_else(|_| "{\"completed\":true}".to_string());
    tokio::fs::write(backup_dir.join(BACKUP_MANIFEST), manifest_str)
        .await
        .with_context(|| {
            format!(
                "写入迁移前备份完成标记失败: {}",
                backup_dir.join(BACKUP_MANIFEST).display()
            )
        })?;

    info!(
        "迁移前备份完成 → {} （共 {} 项，已写入 {}）",
        backup_dir.display(),
        copied.len(),
        BACKUP_MANIFEST
    );

    // 记录本版本已备份；失败仅告警（下次启动会再备份一次，数量仍受清理约束）
    if let Err(e) = write_backup_version(config_dir, app_version).await {
        warn!("写入迁移前备份版本标记失败: {e:#}");
    }
    let removed = prune_pre_migration_backups(config_dir, &backup_dir, PRE_MIGRATION_KEEP).await;
    if !removed.is_empty() {
        info!(
            "迁移前备份保留最近 {} 份，已清理 {} 份旧备份",
            PRE_MIGRATION_KEEP,
            removed.len()
        );
    }

    Ok(PreMigrationBackup {
        dir: backup_dir,
        copied,
    })
}

/// 在 `db_path` 同名基础上拼接边车后缀（`-wal` / `-shm`）。
fn sidecar_path(db_path: &Path, ext: &str) -> PathBuf {
    let mut s = db_path.as_os_str().to_owned();
    s.push(ext);
    PathBuf::from(s)
}

/// 复制单个文件；源不存在则静默跳过，失败仅告警。成功则把目标路径加入 `copied`。
async fn copy_file_best_effort(src: &Path, dest: &Path, copied: &mut Vec<String>) {
    if !src.exists() {
        return;
    }
    if let Some(parent) = dest.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            warn!("迁移前备份：创建目录 {} 失败（{}）", parent.display(), e);
            return;
        }
    }
    match tokio::fs::copy(src, dest).await {
        Ok(_) => copied.push(dest.display().to_string()),
        Err(e) => warn!("迁移前备份：复制 {} 失败（{}）", src.display(), e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn backup_copies_config_files_db_and_wal_dir() {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        let wal_dir = root.path().join("wal");
        tokio::fs::create_dir_all(&config_dir).await.unwrap();
        tokio::fs::create_dir_all(&wal_dir).await.unwrap();

        // 真实 DB 路径在 config 下，且带 -wal 边车文件
        let db_path = config_dir.join("baidu-pcs.db");
        tokio::fs::write(&db_path, b"db").await.unwrap();
        tokio::fs::write(config_dir.join("baidu-pcs.db-wal"), b"wal")
            .await
            .unwrap();
        // 清单中的部分文件存在、部分缺失（缺失应被静默跳过）
        tokio::fs::write(config_dir.join("app.toml"), b"x")
            .await
            .unwrap();
        tokio::fs::write(config_dir.join("accounts.json"), b"x")
            .await
            .unwrap();
        // session.json 不存在 → 跳过
        // wal 目录内放一个 .meta
        tokio::fs::write(wal_dir.join("task1.meta"), b"m")
            .await
            .unwrap();

        let out = create_pre_migration_backup(&config_dir, &db_path, &wal_dir, "1.0.0")
            .await
            .unwrap();

        assert!(out.dir.exists());
        assert!(out.dir.join("app.toml").exists());
        assert!(out.dir.join("accounts.json").exists());
        assert!(out.dir.join("baidu-pcs.db").exists());
        assert!(out.dir.join("baidu-pcs.db-wal").exists());
        assert!(!out.dir.join("session.json").exists(), "缺失文件不应被创建");
        // 整个 wal 目录被复制
        assert!(out.dir.join("wal").join("task1.meta").exists());
    }

    #[tokio::test]
    async fn needs_backup_is_gated_per_app_version() {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let db_path = config_dir.join("baidu-pcs.db");
        let session_path = config_dir.join("session.json");
        let accounts_path = config_dir.join("accounts.json");
        let wal_dir = root.path().join("wal");
        let needs = |v: &str| {
            needs_pre_migration_backup(&config_dir, &db_path, &session_path, &accounts_path, v)
        };

        // 全新安装：无既有数据 → 不备份
        assert!(!needs("1.0.0"));

        // 存在既有数据但无版本标记（从引入标记之前的版本升级）→ 需要备份；
        // 即使已有旧的有效备份也不能跳过（issue：首次升级的备份导致后续升级都不再备份）
        std::fs::write(&db_path, b"db").unwrap();
        let legacy = config_dir
            .join("backups")
            .join("pre_migration_20240101_000000");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join(BACKUP_MANIFEST), b"{}").unwrap();
        assert!(needs("1.0.0"), "旧版本留下的备份不应让后续升级跳过备份");

        // 本版本备份成功 → 同版本后续启动跳过
        create_pre_migration_backup(&config_dir, &db_path, &wal_dir, "1.0.0")
            .await
            .unwrap();
        assert!(!needs("1.0.0"));

        // 升级 / 降级（版本变化）→ 再次需要备份
        assert!(needs("1.1.0"));
        assert!(needs("0.9.0"));
    }

    #[tokio::test]
    async fn fresh_install_records_version_and_skips_second_start() {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let db_path = config_dir.join("baidu-pcs.db");
        let session_path = config_dir.join("session.json");
        let accounts_path = config_dir.join("accounts.json");

        // 首次启动无数据 → 不备份，但记录版本
        assert!(!needs_pre_migration_backup(
            &config_dir,
            &db_path,
            &session_path,
            &accounts_path,
            "1.0.0"
        ));
        ensure_backup_version_recorded(&config_dir, "1.0.0").await;

        // 运行中产生了数据，第二次启动同版本 → 不应误判为升级
        std::fs::write(&db_path, b"db").unwrap();
        assert!(!needs_pre_migration_backup(
            &config_dir,
            &db_path,
            &session_path,
            &accounts_path,
            "1.0.0"
        ));
    }

    #[tokio::test]
    async fn failed_backup_does_not_record_version() {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        // db_path 指向不存在的文件、其余也都不存在 → 一项都没复制 → Err
        let db_path = config_dir.join("baidu-pcs.db");
        let wal_dir = root.path().join("wal");
        assert!(
            create_pre_migration_backup(&config_dir, &db_path, &wal_dir, "1.0.0")
                .await
                .is_err()
        );
        assert_eq!(
            read_backup_version(&config_dir),
            None,
            "备份失败不应写版本标记"
        );
    }

    /// 在 backups/ 下造一个有效备份目录（带空 manifest）。
    fn make_valid_backup(config_dir: &Path, name: &str) -> PathBuf {
        let dir = config_dir.join("backups").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(BACKUP_MANIFEST), b"{}").unwrap();
        dir
    }

    #[tokio::test]
    async fn prune_keeps_latest_valid_backups_and_current() {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        let b1 = make_valid_backup(&config_dir, "pre_migration_20260101_000000");
        let b2 = make_valid_backup(&config_dir, "pre_migration_20260201_000000");
        let b3 = make_valid_backup(&config_dir, "pre_migration_20260301_000000");
        let b4 = make_valid_backup(&config_dir, "pre_migration_20260401_000000");
        // 残缺目录（无 manifest）
        let broken = config_dir
            .join("backups")
            .join("pre_migration_20260501_000000");
        std::fs::create_dir_all(&broken).unwrap();
        // 无法识别的目录 / 文件：不动
        let renamed = make_valid_backup(&config_dir, "pre_migration_before_v2");
        let other = make_valid_backup(&config_dir, "manual_backup");
        // 不定长时间戳（chrono 可解析但破坏字典序）：同样视为无法识别
        let short_ts = make_valid_backup(&config_dir, "pre_migration_2026011_000000");
        // current 的时间戳比其它都旧（模拟时钟回拨）→ 仍必须保留
        let current = make_valid_backup(&config_dir, "pre_migration_20250101_000000");

        let removed = prune_pre_migration_backups(&config_dir, &current, 3).await;

        assert!(current.exists(), "刚创建的备份必须保留");
        assert!(
            b4.exists() && b3.exists(),
            "除 current 外保留最新的 2 份有效备份"
        );
        assert!(!b2.exists() && !b1.exists(), "更早的有效备份应被清理");
        assert!(!broken.exists(), "残缺备份应被清理");
        assert!(
            renamed.exists() && other.exists() && short_ts.exists(),
            "无法识别的目录不应被删除"
        );
        assert_eq!(removed.len(), 3);
    }

    #[tokio::test]
    async fn successful_backup_prunes_old_ones() {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let db_path = config_dir.join("baidu-pcs.db");
        std::fs::write(&db_path, b"db").unwrap();
        let wal_dir = root.path().join("wal");
        for name in [
            "pre_migration_20200101_000000",
            "pre_migration_20200201_000000",
            "pre_migration_20200301_000000",
        ] {
            make_valid_backup(&config_dir, name);
        }

        let out = create_pre_migration_backup(&config_dir, &db_path, &wal_dir, "2.0.0")
            .await
            .unwrap();

        let mut left: Vec<String> = std::fs::read_dir(config_dir.join("backups"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(BACKUP_DIR_PREFIX))
            .collect();
        left.sort();
        assert_eq!(left.len(), PRE_MIGRATION_KEEP);
        assert!(!left.contains(&"pre_migration_20200101_000000".to_string()));
        assert!(out.dir.exists());
        assert_eq!(read_backup_version(&config_dir).as_deref(), Some("2.0.0"));
        let raw = std::fs::read_to_string(out.dir.join(BACKUP_MANIFEST)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(json["app_version"].as_str(), Some("2.0.0"));
    }

    #[tokio::test]
    async fn backup_writes_manifest_and_gates_validity() {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        // 既有数据：DB + -wal/-shm 边车 + session.json + wal/ 任务目录
        let db_path = config_dir.join("baidu-pcs.db");
        std::fs::write(&db_path, b"db").unwrap();
        std::fs::write(config_dir.join("baidu-pcs.db-wal"), b"wal").unwrap();
        std::fs::write(config_dir.join("baidu-pcs.db-shm"), b"shm").unwrap();
        std::fs::write(config_dir.join("session.json"), b"{}").unwrap();
        let wal_dir = root.path().join("wal");
        std::fs::create_dir_all(wal_dir.join("sub")).unwrap();
        std::fs::write(wal_dir.join("task1.meta"), b"m").unwrap();
        std::fs::write(wal_dir.join("sub").join("task2.meta"), b"m").unwrap();

        let out = create_pre_migration_backup(&config_dir, &db_path, &wal_dir, "1.0.0")
            .await
            .expect("应成功并写入 manifest");
        // manifest 必须作为完成标记存在
        assert!(out.dir.join(BACKUP_MANIFEST).exists());
        assert!(!out.copied.is_empty());
        // manifest 应记录 required：DB + -wal/-shm 边车 + session.json + wal 目录（源端均存在）
        let raw = std::fs::read_to_string(out.dir.join(BACKUP_MANIFEST)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let required: Vec<String> = json["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        for expect in [
            "baidu-pcs.db",
            "baidu-pcs.db-wal",
            "baidu-pcs.db-shm",
            "session.json",
            "wal",
        ] {
            assert!(
                required.contains(&expect.to_string()),
                "required 应含 {expect}"
            );
        }
        // manifest 应记录 wal 目录名与其下文件相对路径清单（含子目录，用 / 分隔）
        assert_eq!(json["wal_backup_dir"].as_str(), Some("wal"));
        let wal_files: Vec<String> = json["wal_files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(wal_files.contains(&"task1.meta".to_string()));
        assert!(wal_files.contains(&"sub/task2.meta".to_string()));
        // 备份已有效 → 后续不再需要备份
        assert!(valid_pre_migration_backup_exists(&config_dir));

        // 删除备份中的 -wal 边车 → 该备份立即失效（即使 manifest 仍在）
        std::fs::remove_file(out.dir.join("baidu-pcs.db-wal")).unwrap();
        assert!(
            !valid_pre_migration_backup_exists(&config_dir),
            "required 的 -wal 边车缺失时备份应判为无效"
        );
        std::fs::write(out.dir.join("baidu-pcs.db-wal"), b"wal").unwrap();
        assert!(valid_pre_migration_backup_exists(&config_dir));

        // 删除 wal 子目录中的具体文件（目录仍在）→ 逐项校验缺失 → 失效
        std::fs::remove_file(out.dir.join("wal").join("sub").join("task2.meta")).unwrap();
        assert!(
            !valid_pre_migration_backup_exists(&config_dir),
            "wal 子目录内具体文件缺失时备份应判为无效"
        );
        std::fs::write(out.dir.join("wal").join("sub").join("task2.meta"), b"m").unwrap();
        assert!(valid_pre_migration_backup_exists(&config_dir));

        // 改名顶替：删 task1.meta、补一个无关同数量文件 → 数量不变但具体文件缺失 → 失效
        std::fs::remove_file(out.dir.join("wal").join("task1.meta")).unwrap();
        std::fs::write(out.dir.join("wal").join("unrelated.meta"), b"m").unwrap();
        assert!(
            !valid_pre_migration_backup_exists(&config_dir),
            "具体 wal 文件被顶替（数量相同）时备份应判为无效"
        );
        // 恢复内容 → 再次有效；删整个 wal/ 目录 → 同样失效（目录存在性校验）
        std::fs::write(out.dir.join("wal").join("task1.meta"), b"m").unwrap();
        assert!(valid_pre_migration_backup_exists(&config_dir));
        std::fs::remove_dir_all(out.dir.join("wal")).unwrap();
        assert!(
            !valid_pre_migration_backup_exists(&config_dir),
            "required 的 wal/ 目录缺失时备份应判为无效"
        );
    }

    #[test]
    fn legacy_wal_file_count_fallback_is_honored() {
        // 向后兼容：旧 manifest（#10）只有 wal_file_count、无 wal_files → 回退到数量校验
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        let dir = config_dir
            .join("backups")
            .join("pre_migration_20230101_000000");
        std::fs::create_dir_all(dir.join("wal")).unwrap();
        std::fs::write(dir.join("wal").join("a.meta"), b"m").unwrap();
        std::fs::write(dir.join("wal").join("b.meta"), b"m").unwrap();
        let manifest = br#"{"required":["wal"],"wal_backup_dir":"wal","wal_file_count":2}"#;
        std::fs::write(dir.join(BACKUP_MANIFEST), manifest).unwrap();
        assert!(valid_pre_migration_backup_exists(&config_dir));

        // 删到数量不足 → 失效
        std::fs::remove_file(dir.join("wal").join("b.meta")).unwrap();
        assert!(!valid_pre_migration_backup_exists(&config_dir));
    }

    #[tokio::test]
    async fn incomplete_wal_copy_fails_backup() {
        // wal 目录内有文件复制失败（此处用一个无读权限的文件模拟）→ 整体备份失败、
        // 不写 manifest、返回 Err，使其不被认作有效备份。
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let db_path = config_dir.join("baidu-pcs.db");
        std::fs::write(&db_path, b"db").unwrap();
        let wal_dir = root.path().join("wal");
        std::fs::create_dir_all(&wal_dir).unwrap();
        let bad = wal_dir.join("locked.meta");
        std::fs::write(&bad, b"x").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o000)).unwrap();
        }

        // 仅当权限确实生效（非 root、文件确不可读）才断言；root 下可读则跳过
        let unreadable = std::fs::read(&bad).is_err();
        let res = create_pre_migration_backup(&config_dir, &db_path, &wal_dir, "1.0.0").await;
        if unreadable {
            assert!(res.is_err(), "wal 内容复制不完整时应返回 Err");
            assert!(
                !valid_pre_migration_backup_exists(&config_dir),
                "复制不完整的备份不应被认作有效"
            );
        }
        // 还原权限以便 tempdir 清理
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o644));
        }
        let _ = res;
    }

    #[test]
    fn legacy_manifest_without_required_is_still_valid() {
        // 向后兼容：旧 manifest 不含 required 字段 → 仍按「存在即有效」处理
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        let dir = config_dir
            .join("backups")
            .join("pre_migration_20230101_000000");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(BACKUP_MANIFEST), br#"{"completed_at":"x"}"#).unwrap();
        assert!(valid_pre_migration_backup_exists(&config_dir));

        // required 为空数组 → 也算有效（无任何关键文件可校验）
        std::fs::write(dir.join(BACKUP_MANIFEST), br#"{"required":[]}"#).unwrap();
        assert!(valid_pre_migration_backup_exists(&config_dir));

        // required 含缺失文件 → 无效
        std::fs::write(dir.join(BACKUP_MANIFEST), br#"{"required":["missing.db"]}"#).unwrap();
        assert!(!valid_pre_migration_backup_exists(&config_dir));
    }

    #[test]
    fn corrupt_or_unreadable_manifest_is_invalid() {
        // 损坏 / 半写入的 manifest（JSON 解析失败）→ 不算有效备份
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        let dir = config_dir
            .join("backups")
            .join("pre_migration_20230101_000000");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(BACKUP_MANIFEST), b"{not valid json").unwrap();
        assert!(
            !valid_pre_migration_backup_exists(&config_dir),
            "损坏的 manifest 不应被认作有效备份"
        );
    }
}

/// 目录递归复制统计：`files` 为成功复制的文件**相对 `dest` 根**的路径（用 `/`
/// 分隔）；`failed` 为复制 / 读取失败的项数（含目录读取、目录遍历、条目类型读取、
/// 单文件复制失败）。`failed > 0` 表示备份**不完整**。
#[derive(Debug, Default, Clone)]
struct CopyDirStats {
    files: Vec<String>,
    failed: usize,
}

impl CopyDirStats {
    fn copied(&self) -> usize {
        self.files.len()
    }
}

/// 递归复制目录，返回 [`CopyDirStats`]。单文件 / 子项失败仅告警、不中断整体，但会
/// 计入 `failed`，由调用方据此判断目录备份是否完整。
async fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<CopyDirStats> {
    tokio::fs::create_dir_all(dest)
        .await
        .with_context(|| format!("创建目录失败: {}", dest.display()))?;
    let mut stats = CopyDirStats::default();
    let dest_root = dest.to_path_buf();
    // 用显式栈避免 async 递归。
    let mut stack: Vec<(PathBuf, PathBuf)> = vec![(src.to_path_buf(), dest.to_path_buf())];
    while let Some((cur_src, cur_dest)) = stack.pop() {
        let mut entries = match tokio::fs::read_dir(&cur_src).await {
            Ok(e) => e,
            Err(e) => {
                warn!("迁移前备份：读取目录 {} 失败（{}）", cur_src.display(), e);
                stats.failed += 1;
                continue;
            }
        };
        // 显式处理 next_entry() 的 Err：目录遍历中途失败必须计入 failed，
        // 否则会把残缺的 wal 备份误判为「完整复制」。
        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(e) => {
                    warn!("迁移前备份：遍历目录 {} 失败（{}）", cur_src.display(), e);
                    stats.failed += 1;
                    break;
                }
            };
            let path = entry.path();
            let target = cur_dest.join(entry.file_name());
            match entry.file_type().await {
                Ok(ft) if ft.is_dir() => {
                    if let Err(e) = tokio::fs::create_dir_all(&target).await {
                        warn!("迁移前备份：创建子目录 {} 失败（{}）", target.display(), e);
                        stats.failed += 1;
                        continue;
                    }
                    stack.push((path, target));
                }
                Ok(_) => match tokio::fs::copy(&path, &target).await {
                    Ok(_) => {
                        let rel = target
                            .strip_prefix(&dest_root)
                            .unwrap_or(&target)
                            .to_string_lossy()
                            .replace('\\', "/");
                        stats.files.push(rel);
                    }
                    Err(e) => {
                        warn!("迁移前备份：复制 {} 失败（{}）", path.display(), e);
                        stats.failed += 1;
                    }
                },
                Err(e) => {
                    warn!("迁移前备份：读取条目类型失败 {}（{}）", path.display(), e);
                    stats.failed += 1;
                }
            }
        }
    }
    Ok(stats)
}
