//! 配置文件原子写入
//!
//! 直接 `fs::write` 会先截断再写：进程在中途被杀、宿主机断电时，磁盘上可能只剩
//! 一个空文件或半截文件，下次启动就读不出配置。这里改为：
//!
//! 1. 在**同一目录**写临时文件并 `fsync`（保证 rename 是同一文件系统内的原子替换）
//! 2. `rename` 覆盖目标文件 —— 任何时刻磁盘上要么是旧内容、要么是新内容
//! 3. Unix 下再 `fsync` 父目录，确保 rename 本身落盘

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// 同进程内并发写同一文件时，临时文件名也不能撞车
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn tmp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{}.tmp.{}.{}", file_name, std::process::id(), seq))
}

/// 原子写入（同步版本，供非 async 上下文使用）
///
/// - 目标是软链接时写入链接指向的文件，链接本身保留
/// - Unix 下沿用原文件的权限与属主（如用户给 auth.json 设的 0600），
///   不因 rename 换成新文件而被重置
/// - 目标是单文件 bind mount（Docker `-v ./app.toml:/app/config/app.toml`）时
///   无法被 rename 替换，退回原地覆盖写，保证仍能保存
pub fn write_atomic_sync(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let target = resolve_symlink(path);
    let existing = std::fs::metadata(&target).ok();

    let parent = match target.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::fs::create_dir_all(&parent)?;

    let tmp = tmp_path_for(&target);
    let result = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        #[cfg(unix)]
        if let Some(meta) = &existing {
            preserve_unix_metadata(&tmp, meta);
        }
        std::fs::rename(&tmp, &target)
    })();

    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        if existing.is_some() && is_unreplaceable_target(&e) {
            warn_in_place_once(&target, &e);
            return write_in_place(&target, contents);
        }
        return Err(e);
    }

    #[cfg(unix)]
    {
        // 目录 fsync 失败不影响文件内容本身的正确性，只降低断电场景的保障
        if let Ok(dir) = std::fs::File::open(&parent) {
            let _ = dir.sync_all();
        }
    }

    Ok(())
}

/// 软链接解析到真实路径；悬空链接按链接内容拼出目标（与 `fs::write` 行为一致）
fn resolve_symlink(path: &Path) -> PathBuf {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => std::fs::canonicalize(path)
            .or_else(|_| {
                std::fs::read_link(path).map(|link| match path.parent() {
                    Some(parent) if link.is_relative() => parent.join(link),
                    _ => link,
                })
            })
            .unwrap_or_else(|_| path.to_path_buf()),
        _ => path.to_path_buf(),
    }
}

/// 尽力而为：部分网络文件系统（如 NAS 的 CIFS 挂载）不支持 chmod / chown，
/// 失败不能阻止保存
#[cfg(unix)]
fn preserve_unix_metadata(tmp: &Path, original: &std::fs::Metadata) {
    use std::os::unix::fs::MetadataExt;

    let _ = std::fs::set_permissions(tmp, original.permissions());
    // 以 root 运行（如容器内）时把属主还原，避免宿主机用户之后无法编辑；
    // 非 root 无权 chown，属主本来就是自己，失败忽略即可
    let _ = std::os::unix::fs::chown(tmp, Some(original.uid()), Some(original.gid()));
}

/// rename 无法替换目标：目标是挂载点（EBUSY）或跨文件系统（EXDEV）
fn is_unreplaceable_target(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::ResourceBusy | std::io::ErrorKind::CrossesDevices
    )
}

fn warn_in_place_once(target: &Path, e: &std::io::Error) {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            "{:?} 无法通过 rename 原子替换（{}），可能是以单文件方式挂载；改为原地覆盖写入。\
             建议挂载整个 config 目录以获得断电保护",
            target,
            e
        );
    }
}

fn write_in_place(target: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(target)?;
    file.write_all(contents)?;
    file.sync_all()
}

/// 是否为本模块生成的临时文件名：`.{原文件名}.tmp.{pid}.{seq}`
fn is_tmp_file_name(name: &str) -> bool {
    let mut parts = name.rsplitn(3, '.');
    let (Some(seq), Some(pid), Some(rest)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let is_num = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    is_num(seq) && is_num(pid) && rest.starts_with('.') && rest.ends_with(".tmp") && rest.len() > 5
}

/// 清理目录下写到一半被打断（进程被强杀 / 断电）残留的临时文件
///
/// 必须在本进程开始写这些文件之前调用：容器里 PID 恒为 1，无法靠 PID 区分
/// 「上次残留」与「本次正在写」。返回删除的文件数。
pub fn cleanup_stale_tmp_files(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_tmp_file_name(name) || !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        match std::fs::remove_file(entry.path()) {
            Ok(()) => {
                removed += 1;
                tracing::info!("已清理残留临时文件: {:?}", entry.path());
            }
            Err(e) => tracing::warn!("清理残留临时文件失败: {:?}, {}", entry.path(), e),
        }
    }
    removed
}

/// 原子写入（async 版本，阻塞 IO 放到 blocking 线程池）
pub async fn write_atomic(
    path: impl AsRef<Path>,
    contents: impl Into<Vec<u8>>,
) -> std::io::Result<()> {
    let path = path.as_ref().to_path_buf();
    let contents = contents.into();
    tokio::task::spawn_blocking(move || write_atomic_sync(&path, &contents))
        .await
        .map_err(std::io::Error::other)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_write_atomic_replaces_content_and_leaves_no_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.toml");

        write_atomic(&path, "a = 1\n").await.unwrap();
        write_atomic(&path, "a = 2\n").await.unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a = 2\n");
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() != "app.toml")
            .collect();
        assert!(leftovers.is_empty(), "残留临时文件: {:?}", leftovers);
    }

    #[cfg(unix)]
    #[test]
    fn test_write_atomic_preserves_permissions_and_symlink() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.json");
        std::fs::write(&real, "old").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.path().join("auth.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        write_atomic_sync(&link, b"new").unwrap();

        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "new");
        assert_eq!(
            std::fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn test_cleanup_stale_tmp_files_only_removes_own_tmp_files() {
        let dir = tempfile::tempdir().unwrap();
        let keep = [
            "app.toml",
            "auth.json",
            ".app.toml.tmp",
            ".app.toml.tmp.1",
            "app.toml.tmp.1.2",
            ".tmp.1.2",
            "accounts.json.tmp.123",
        ];
        for name in keep {
            std::fs::write(dir.path().join(name), "x").unwrap();
        }
        // 模拟上次被强杀残留：写到一半的临时文件
        std::fs::write(dir.path().join(".app.toml.tmp.1.0"), "half").unwrap();
        std::fs::write(dir.path().join(".auth.json.tmp.4242.17"), "half").unwrap();

        assert_eq!(cleanup_stale_tmp_files(dir.path()), 2);
        for name in keep {
            assert!(dir.path().join(name).exists(), "{name} 不应被删除");
        }
        assert!(!dir.path().join(".app.toml.tmp.1.0").exists());
        assert!(!dir.path().join(".auth.json.tmp.4242.17").exists());
    }

    #[test]
    fn test_write_atomic_sync_creates_parent_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("auth.json");

        write_atomic_sync(&path, b"{}").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"{}");
    }

    #[test]
    fn test_write_atomic_failure_cleans_tmp_and_keeps_target() {
        let dir = tempfile::tempdir().unwrap();

        // 目标是非空目录 → rename 必然失败，目标不应被破坏、临时文件不应残留
        let target_dir = dir.path().join("as_dir");
        std::fs::create_dir(&target_dir).unwrap();
        std::fs::write(target_dir.join("keep"), "x").unwrap();
        assert!(write_atomic_sync(&target_dir, b"new").is_err());

        assert!(target_dir.join("keep").exists());
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names.iter().all(|n| !n.contains(".tmp.")),
            "失败后残留临时文件: {:?}",
            names
        );
    }
}
