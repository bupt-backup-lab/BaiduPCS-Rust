//! OpenList 联动配置
//!
//! 保存于 `config/app.toml` 的 `[openlist]` 段，
//! 负责 BaiduPCS-Rust 与 OpenList 的服务地址、凭证以及各百度账号的挂载路径映射。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OpenList 账号挂载路径映射
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OpenListAccountMapping {
    /// OpenList 挂载路径，如 "/HKU慕容" 或 "/juna"
    pub mount: String,
    /// 对应的网盘根目录，默认 "/"
    #[serde(default = "default_root")]
    pub root: Option<String>,
    /// 可选：该账号独立使用的 OpenList 地址
    #[serde(default)]
    pub base: Option<String>,
}

fn default_root() -> Option<String> {
    Some("/".to_string())
}

/// OpenList 联动配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenListConfig {
    /// 是否启用 OpenList 联动
    #[serde(default)]
    pub enabled: bool,
    /// OpenList 基础地址，如 "https://openlist.example.com:11443"
    #[serde(default, alias = "baseUrl")]
    pub base_url: String,
    /// OpenList 具有刷新/写入权限的专用用户名
    #[serde(default)]
    pub username: String,
    /// OpenList 密码
    #[serde(default)]
    pub password: String,
    /// 桌面端自定义协议名，用于唤起 mpv，默认 "mpvpcs"
    #[serde(default = "default_desktop_scheme", alias = "desktopScheme")]
    pub desktop_scheme: String,
    /// 未手动配置挂载路径时，是否自动按 "/用户名" 猜测
    #[serde(default = "default_true", alias = "autoMountByUsername")]
    pub auto_mount_by_username: bool,
    /// 无法识别账号时的兜底挂载路径
    #[serde(default, alias = "fallbackMount")]
    pub fallback_mount: String,
    /// 操作接管模式: "off" (关闭), "observe" (观察), "on" (接管)
    /// 开启接管后，同网盘内的新建文件夹/重命名/删除/移动/复制 操作将改由 OpenList 执行
    #[serde(default = "default_takeover_mode", alias = "takeoverMode")]
    pub takeover_mode: String,
    /// 账号挂载路径映射表：百度用户名或 UID -> 挂载配置
    #[serde(default)]
    pub accounts: HashMap<String, OpenListAccountMapping>,
}

fn default_takeover_mode() -> String {
    "observe".to_string()
}

fn default_true() -> bool {
    true
}

fn default_desktop_scheme() -> String {
    "mpvpcs".to_string()
}

impl Default for OpenListConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: String::new(),
            username: String::new(),
            password: String::new(),
            desktop_scheme: default_desktop_scheme(),
            auto_mount_by_username: true,
            fallback_mount: String::new(),
            takeover_mode: default_takeover_mode(),
            accounts: HashMap::new(),
        }
    }
}
