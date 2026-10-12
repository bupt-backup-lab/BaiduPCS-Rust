//! 系统本地操作 API：在运行后端的机器上用系统文件管理器打开目录
//!
//! 仅当「后端运行在桌面系统」且「请求来自本机（loopback）」时可用：
//! - Docker / 无图形界面的 Linux 上没有可唤起的文件管理器；
//! - 远程浏览器访问时，窗口会弹在服务器上而不是用户眼前，没有意义。
//!
//! 不满足条件时前端回退为展示路径 + 复制。

use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::process::Command;

use axum::{extract::ConnectInfo, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::config::EnvDetector;
use crate::server::handlers::ApiResponse;

/// 打开文件夹能力检测结果
#[derive(Debug, Serialize)]
pub struct OpenFolderCapability {
    /// 是否可以直接唤起系统文件管理器
    pub supported: bool,
    /// 不支持时的原因（展示给用户）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// 打开文件夹请求
#[derive(Debug, Deserialize)]
pub struct OpenFolderRequest {
    /// 本地绝对路径；为文件时打开其所在目录并选中该文件
    pub path: String,
}

/// 是否为本机地址（兼容 IPv4-mapped IPv6，如 `::ffff:127.0.0.1`）
fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6
            .to_ipv4_mapped()
            .map(|v4| v4.is_loopback())
            .unwrap_or_else(|| v6.is_loopback()),
    }
}

/// 后端所在环境是否有可唤起的图形文件管理器
fn desktop_unavailable_reason() -> Option<&'static str> {
    if EnvDetector::is_docker() {
        return Some("当前运行在 Docker 容器中，无法打开系统文件管理器");
    }
    if cfg!(any(target_os = "windows", target_os = "macos")) {
        return None;
    }
    if cfg!(target_os = "linux") {
        let has_display = ["DISPLAY", "WAYLAND_DISPLAY"]
            .iter()
            .any(|k| std::env::var_os(k).is_some_and(|v| !v.is_empty()));
        if has_display {
            return None;
        }
        return Some("当前系统没有图形界面，无法打开文件管理器");
    }
    Some("当前系统不支持打开文件管理器")
}

/// 请求是否来自本机：TCP 对端须为 loopback；经同机反向代理转发时，
/// 还要求代理头（X-Forwarded-For 首跳 / X-Real-IP）记录的原始客户端也是 loopback
fn is_local_request(peer: Option<SocketAddr>, headers: &HeaderMap) -> bool {
    if !peer.is_some_and(|addr| is_loopback(addr.ip())) {
        return false;
    }
    let forwarded_client = |name: &str| {
        headers.get(name).map(|v| {
            v.to_str()
                .ok()
                .and_then(|s| s.split(',').next())
                .and_then(|s| s.trim().parse::<IpAddr>().ok())
        })
    };
    for name in ["x-forwarded-for", "x-real-ip"] {
        // 有代理头但解析不出 loopback 地址 → 视为远程客户端
        if let Some(ip) = forwarded_client(name) {
            if !ip.is_some_and(is_loopback) {
                return false;
            }
        }
    }
    true
}

fn check_capability(peer: Option<SocketAddr>, headers: &HeaderMap) -> Result<(), String> {
    if let Some(reason) = desktop_unavailable_reason() {
        return Err(reason.to_string());
    }
    if is_local_request(peer, headers) {
        Ok(())
    } else {
        Err("仅在运行本程序的电脑上通过 localhost / 127.0.0.1 访问时可用".to_string())
    }
}

/// GET /api/v1/system/open-folder
pub async fn get_open_folder_capability(
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
) -> Json<ApiResponse<OpenFolderCapability>> {
    let result = check_capability(peer.map(|c| c.0), &headers);
    Json(ApiResponse::success(OpenFolderCapability {
        supported: result.is_ok(),
        reason: result.err(),
    }))
}

/// POST /api/v1/system/open-folder
pub async fn open_local_folder(
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(req): Json<OpenFolderRequest>,
) -> Json<ApiResponse<()>> {
    if let Err(reason) = check_capability(peer.map(|c| c.0), &headers) {
        return Json(ApiResponse::error(403, reason));
    }

    let raw = req.path.trim();
    if raw.is_empty() {
        return Json(ApiResponse::error(400, "路径不能为空".to_string()));
    }
    // 下载目录可配置为相对路径（相对后端工作目录），任务里的本地路径随之为相对路径
    let path = match std::path::absolute(raw) {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::error(400, format!("无效路径: {}", e))),
    };
    let path = path.as_path();
    let is_dir = match tokio::fs::metadata(path).await {
        Ok(meta) => meta.is_dir(),
        Err(_) => return Json(ApiResponse::error(404, format!("路径不存在: {}", raw))),
    };

    match spawn_file_manager(path, is_dir) {
        Ok(()) => {
            info!("已打开本地文件夹: {}", raw);
            Json(ApiResponse::success(()))
        }
        Err(e) => {
            warn!("打开本地文件夹失败: path={}, error={}", raw, e);
            Json(ApiResponse::error(500, format!("打开文件夹失败: {}", e)))
        }
    }
}

/// 唤起系统文件管理器：目录直接打开；文件则打开所在目录并尽量选中该文件
fn spawn_file_manager(path: &Path, is_dir: bool) -> std::io::Result<()> {
    let mut cmd = build_command(path, is_dir)?;
    let mut child = cmd.spawn()?;
    // 后台回收子进程，避免 Unix 下残留僵尸进程；explorer 的退出码不可靠，不检查
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(target_os = "windows")]
fn build_command(path: &Path, is_dir: bool) -> std::io::Result<Command> {
    use std::os::windows::process::CommandExt;

    // explorer 不走标准命令行解析：`/select,` 与路径之间不能有空格，路径需整体加引号，
    // 必须用 raw_arg 原样传递。Windows 路径不允许包含 `"`，这里兜底拒绝。
    let mut p = path.to_string_lossy().replace('/', "\\");
    if p.contains('"') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "路径包含非法字符",
        ));
    }
    // 去掉目录末尾的反斜杠（盘符根目录 `C:\` 除外），避免 `"D:\dir\"` 中 `\"` 被当成转义
    while p.len() > 3 && p.ends_with('\\') {
        p.pop();
    }
    let mut cmd = Command::new("explorer");
    if is_dir {
        cmd.raw_arg(format!("\"{}\"", p));
    } else {
        cmd.raw_arg(format!("/select,\"{}\"", p));
    }
    Ok(cmd)
}

#[cfg(target_os = "macos")]
fn build_command(path: &Path, is_dir: bool) -> std::io::Result<Command> {
    let mut cmd = Command::new("open");
    if !is_dir {
        // -R：在 Finder 中显示并选中
        cmd.arg("-R");
    }
    cmd.arg(path);
    Ok(cmd)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn build_command(path: &Path, is_dir: bool) -> std::io::Result<Command> {
    // xdg-open 不支持选中文件，文件时打开其所在目录
    let dir = if is_dir {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    let mut cmd = Command::new("xdg-open");
    cmd.arg(dir);
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_is_loopback() {
        assert!(is_loopback(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(is_loopback(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(is_loopback(IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped())));
        assert!(!is_loopback(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2))));
        assert!(!is_loopback(IpAddr::V6(
            Ipv4Addr::new(192, 168, 1, 2).to_ipv6_mapped()
        )));
    }

    #[test]
    fn test_is_local_request() {
        let local: SocketAddr = "127.0.0.1:50000".parse().unwrap();
        let remote: SocketAddr = "192.168.1.2:50000".parse().unwrap();
        let empty = HeaderMap::new();

        assert!(is_local_request(Some(local), &empty));
        assert!(!is_local_request(Some(remote), &empty));
        assert!(!is_local_request(None, &empty));

        // 同机反向代理转发的远程客户端
        let mut proxied = HeaderMap::new();
        proxied.insert("x-forwarded-for", "192.168.1.9, 127.0.0.1".parse().unwrap());
        assert!(!is_local_request(Some(local), &proxied));

        let mut proxied_local = HeaderMap::new();
        proxied_local.insert("x-forwarded-for", "127.0.0.1".parse().unwrap());
        proxied_local.insert("x-real-ip", "::1".parse().unwrap());
        assert!(is_local_request(Some(local), &proxied_local));

        let mut garbage = HeaderMap::new();
        garbage.insert("x-real-ip", "unknown".parse().unwrap());
        assert!(!is_local_request(Some(local), &garbage));
    }
}
