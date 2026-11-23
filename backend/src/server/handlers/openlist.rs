//! OpenList 联动 API
//!
//! 提供 OpenList 配置读写（存入 `config/app.toml`），
//! 以及由后端代理发起的连通性测试与目录缓存刷新（完全避开浏览器 CORS 跨域限制）。

use crate::config::{OpenListAccountMapping, OpenListConfig, APP_CONFIG_PATH};
use crate::server::error::{ApiError, ApiResult};
use crate::server::handlers::ApiResponse;
use crate::server::AppState;
use axum::{extract::State, response::Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::info;

/// OpenList 配置响应（密码脱敏）
#[derive(Debug, Serialize)]
pub struct OpenListConfigResponse {
    pub enabled: bool,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    pub username: String,
    /// 密码脱敏：前端不直接展示真实密码
    pub password: String,
    #[serde(rename = "hasPassword")]
    pub has_password: bool,
    #[serde(rename = "desktopScheme")]
    pub desktop_scheme: String,
    #[serde(rename = "autoMountByUsername")]
    pub auto_mount_by_username: bool,
    #[serde(rename = "fallbackMount")]
    pub fallback_mount: String,
    pub accounts: HashMap<String, OpenListAccountMapping>,
}

/// OpenList 配置更新请求
#[derive(Debug, Deserialize)]
pub struct OpenListConfigUpdateRequest {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, alias = "baseUrl")]
    pub base_url: String,
    #[serde(default)]
    pub username: String,
    /// 若为空字符串或 None，表示保留原密码不变
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default = "default_desktop_scheme", alias = "desktopScheme")]
    pub desktop_scheme: String,
    #[serde(default = "default_true", alias = "autoMountByUsername")]
    pub auto_mount_by_username: bool,
    #[serde(default, alias = "fallbackMount")]
    pub fallback_mount: String,
    #[serde(default)]
    pub accounts: HashMap<String, OpenListAccountMapping>,
}

fn default_true() -> bool {
    true
}

fn default_desktop_scheme() -> String {
    "mpvpcs".to_string()
}

/// GET /config/openlist
/// 获取 OpenList 配置（密码脱敏）
pub async fn get_openlist_config(
    State(app_state): State<AppState>,
) -> ApiResult<Json<ApiResponse<OpenListConfigResponse>>> {
    let cfg = app_state.config.read().await.openlist.clone();
    let has_password = !cfg.password.is_empty();

    Ok(Json(ApiResponse::success(OpenListConfigResponse {
        enabled: cfg.enabled,
        base_url: cfg.base_url,
        username: cfg.username,
        password: String::new(), // 脱敏为空字符串
        has_password,
        desktop_scheme: cfg.desktop_scheme,
        auto_mount_by_username: cfg.auto_mount_by_username,
        fallback_mount: cfg.fallback_mount,
        accounts: cfg.accounts,
    })))
}

/// PUT /config/openlist
/// 更新 OpenList 配置（原子落盘保存至 app.toml）
pub async fn update_openlist_config(
    State(app_state): State<AppState>,
    Json(req): Json<OpenListConfigUpdateRequest>,
) -> ApiResult<Json<ApiResponse<OpenListConfigResponse>>> {
    info!(
        "更新 OpenList 配置: enabled={}, baseUrl={}, username={}",
        req.enabled, req.base_url, req.username
    );

    let mut guard = app_state.config.write().await;
    let mut config = guard.clone();

    // 密码处理：若请求中提供了非空新密码则更新，否则保持原密码
    let password = match req.password {
        Some(ref p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => guard.openlist.password.clone(),
    };

    let new_openlist = OpenListConfig {
        enabled: req.enabled,
        base_url: req.base_url.trim().trim_end_matches('/').to_string(),
        username: req.username.trim().to_string(),
        password,
        desktop_scheme: if req.desktop_scheme.trim().is_empty() {
            default_desktop_scheme()
        } else {
            req.desktop_scheme.trim().to_string()
        },
        auto_mount_by_username: req.auto_mount_by_username,
        fallback_mount: req.fallback_mount.trim().to_string(),
        accounts: req.accounts,
    };

    config.openlist = new_openlist.clone();
    config
        .save_to_file(APP_CONFIG_PATH)
        .await
        .map_err(ApiError::Internal)?;

    *guard = config;

    let has_password = !new_openlist.password.is_empty();
    Ok(Json(ApiResponse::success(OpenListConfigResponse {
        enabled: new_openlist.enabled,
        base_url: new_openlist.base_url,
        username: new_openlist.username,
        password: String::new(),
        has_password,
        desktop_scheme: new_openlist.desktop_scheme,
        auto_mount_by_username: new_openlist.auto_mount_by_username,
        fallback_mount: new_openlist.fallback_mount,
        accounts: new_openlist.accounts,
    })))
}

/// 连通性测试请求体
#[derive(Debug, Default, Deserialize)]
pub struct OpenListTestRequest {
    #[serde(default, alias = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

/// 连通性测试响应体
#[derive(Debug, Serialize)]
pub struct OpenListTestResponse {
    pub success: bool,
    pub message: String,
}

/// 创建带局域网宽松策略的 HTTP 客户端
fn make_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true) // 兼容 NAS / 局域网自签名证书
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// POST /openlist/test
/// 由后端代理测试与 OpenList 服务的连通性
pub async fn test_openlist_connection(
    State(app_state): State<AppState>,
    Json(req): Json<OpenListTestRequest>,
) -> ApiResult<Json<ApiResponse<OpenListTestResponse>>> {
    let saved = app_state.config.read().await.openlist.clone();

    let base_url = req
        .base_url
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| saved.base_url.clone())
        .trim_end_matches('/')
        .to_string();

    if base_url.is_empty() {
        return Ok(Json(ApiResponse::success(OpenListTestResponse {
            success: false,
            message: "OpenList 网址不能为空".to_string(),
        })));
    }

    let username = req
        .username
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| saved.username.clone());

    let password = req
        .password
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| saved.password.clone());

    let client = make_http_client();

    if !username.is_empty() && !password.is_empty() {
        // 测试账号密码登录
        let login_url = format!("{}/api/auth/login", base_url);
        let body = serde_json::json!({
            "username": username,
            "password": password,
        });

        match client.post(&login_url).json(&body).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json.get("code").and_then(|c| c.as_i64()) == Some(200) {
                        return Ok(Json(ApiResponse::success(OpenListTestResponse {
                            success: true,
                            message: "OpenList 连接与账号认证成功！".to_string(),
                        })));
                    } else {
                        let msg = json
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("登录失败，请检查账号密码");
                        return Ok(Json(ApiResponse::success(OpenListTestResponse {
                            success: false,
                            message: format!("OpenList 认证失败: {}", msg),
                        })));
                    }
                }
                Ok(Json(ApiResponse::success(OpenListTestResponse {
                    success: false,
                    message: "OpenList 返回了非预期响应".to_string(),
                })))
            }
            Err(e) => Ok(Json(ApiResponse::success(OpenListTestResponse {
                success: false,
                message: format!("连接 OpenList 失败: {}", e),
            }))),
        }
    } else {
        // 访客连通性测试
        let list_url = format!("{}/api/fs/list", base_url);
        let body = serde_json::json!({
            "path": "/",
            "page": 1,
            "per_page": 1,
        });

        match client.post(&list_url).json(&body).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    Ok(Json(ApiResponse::success(OpenListTestResponse {
                        success: true,
                        message: "OpenList 服务连接正常（访客模式）".to_string(),
                    })))
                } else {
                    Ok(Json(ApiResponse::success(OpenListTestResponse {
                        success: false,
                        message: format!("OpenList HTTP 状态码: {}", resp.status()),
                    })))
                }
            }
            Err(e) => Ok(Json(ApiResponse::success(OpenListTestResponse {
                success: false,
                message: format!("连接 OpenList 失败: {}", e),
            }))),
        }
    }
}

/// 目录缓存刷新请求体
#[derive(Debug, Deserialize)]
pub struct OpenListRefreshRequest {
    /// 网盘目录路径，如 "/我的电影/动漫"
    pub dir: String,
    /// 可选：账号 UID
    #[serde(default)]
    pub uid: Option<u64>,
    /// 可选：账号用户名
    #[serde(default)]
    pub username: Option<String>,
}

/// 目录缓存刷新响应体
#[derive(Debug, Serialize)]
pub struct OpenListRefreshResponse {
    pub success: bool,
    pub total: usize,
    pub message: String,
}

/// POST /openlist/refresh
/// 由后端代理向 OpenList 发起目录强制刷新请求
pub async fn refresh_openlist_dir(
    State(app_state): State<AppState>,
    Json(req): Json<OpenListRefreshRequest>,
) -> ApiResult<Json<ApiResponse<OpenListRefreshResponse>>> {
    let cfg = app_state.config.read().await.openlist.clone();

    if !cfg.enabled {
        return Err(ApiError::BadRequest(
            "未启用 OpenList 联动，请先在系统设置中开启".to_string(),
        ));
    }
    if cfg.base_url.is_empty() {
        return Err(ApiError::BadRequest("未配置 OpenList 服务网址".to_string()));
    }
    if cfg.username.is_empty() || cfg.password.is_empty() {
        return Err(ApiError::BadRequest(
            "未配置 OpenList 刷新账号密码（刷新需要写入权限的专用账号）".to_string(),
        ));
    }

    // 匹配账号挂载路径
    let mut mount_path = String::new();
    let mut base_url = cfg.base_url.clone();
    let mut root_path = "/".to_string();

    // 1. 尝试按 uid 查映射
    if let Some(uid) = req.uid {
        let uid_str = uid.to_string();
        if let Some(m) = cfg.accounts.get(&uid_str) {
            mount_path = m.mount.clone();
            if let Some(ref r) = m.root {
                root_path = r.clone();
            }
            if let Some(ref b) = m.base {
                base_url = b.trim_end_matches('/').to_string();
            }
        }
    }

    // 2. 尝试按用户名查映射
    if mount_path.is_empty() {
        if let Some(ref u) = req.username {
            if let Some(m) = cfg.accounts.get(u) {
                mount_path = m.mount.clone();
                if let Some(ref r) = m.root {
                    root_path = r.clone();
                }
                if let Some(ref b) = m.base {
                    base_url = b.trim_end_matches('/').to_string();
                }
            }
        }
    }

    // 3. 尝试自动按 /{用户名} 推导
    if mount_path.is_empty() && cfg.auto_mount_by_username {
        if let Some(ref u) = req.username {
            mount_path = format!("/{}", u.trim_matches('/'));
        }
    }

    // 4. 尝试兜底挂载路径
    if mount_path.is_empty() && !cfg.fallback_mount.is_empty() {
        mount_path = cfg.fallback_mount.clone();
    }

    if mount_path.is_empty() {
        return Err(ApiError::BadRequest(
            "未找到对应账号的 OpenList 挂载路径，请在设置中指定".to_string(),
        ));
    }

    // 计算相对于 root 的相对路径
    let dir_clean = req.dir.trim();
    let root_clean = root_path.trim().trim_end_matches('/');

    let rel_path = if root_clean.is_empty() || root_clean == "/" {
        dir_clean.trim_start_matches('/')
    } else if let Some(stripped) = dir_clean.strip_prefix(root_clean) {
        stripped.trim_start_matches('/')
    } else {
        return Err(ApiError::BadRequest(format!(
            "当前目录「{}」不在 OpenList 挂载的根目录「{}」下",
            dir_clean, root_path
        )));
    };

    let mount_clean = mount_path.trim().trim_end_matches('/');
    let target_path = if rel_path.is_empty() {
        mount_clean.to_string()
    } else {
        format!("{}/{}", mount_clean, rel_path)
    };

    info!(
        "向 OpenList 发起目录刷新: base_url={}, target_path={}",
        base_url, target_path
    );

    let client = make_http_client();

    // 登录获取 Token
    let login_url = format!("{}/api/auth/login", base_url);
    let login_body = serde_json::json!({
        "username": cfg.username,
        "password": cfg.password,
    });

    let token = match client.post(&login_url).json(&login_body).send().await {
        Ok(resp) => {
            let json = resp
                .json::<serde_json::Value>()
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("解析登录响应失败: {}", e)))?;
            if json.get("code").and_then(|c| c.as_i64()) != Some(200) {
                let msg = json
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("认证失败");
                return Err(ApiError::BadRequest(format!("OpenList 登录失败: {}", msg)));
            }
            json.get("data")
                .and_then(|d| d.get("token"))
                .and_then(|t| t.as_str())
                .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("OpenList 未返回有效 Token")))?
                .to_string()
        }
        Err(e) => {
            return Err(ApiError::Internal(anyhow::anyhow!(
                "请求 OpenList 登录接口失败: {}",
                e
            )));
        }
    };

    // 触发目录刷新
    let list_url = format!("{}/api/fs/list", base_url);
    let list_body = serde_json::json!({
        "path": target_path,
        "page": 1,
        "per_page": 1,
        "refresh": true,
    });

    match client
        .post(&list_url)
        .header("Authorization", token)
        .json(&list_body)
        .send()
        .await
    {
        Ok(resp) => {
            let json = resp
                .json::<serde_json::Value>()
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("解析刷新响应失败: {}", e)))?;
            if json.get("code").and_then(|c| c.as_i64()) != Some(200) {
                let msg = json
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("刷新失败");
                return Err(ApiError::BadRequest(format!("OpenList 刷新失败: {}", msg)));
            }
            let total = json
                .get("data")
                .and_then(|d| d.get("total"))
                .and_then(|t| t.as_u64())
                .unwrap_or(0) as usize;

            Ok(Json(ApiResponse::success(OpenListRefreshResponse {
                success: true,
                total,
                message: format!("目录刷新成功，共 {} 项", total),
            })))
        }
        Err(e) => Err(ApiError::Internal(anyhow::anyhow!(
            "请求 OpenList 刷新接口失败: {}",
            e
        ))),
    }
}
