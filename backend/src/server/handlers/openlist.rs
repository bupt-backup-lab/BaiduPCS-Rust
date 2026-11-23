//! OpenList 联动 API
//!
//! 提供 OpenList 配置读写（存入 `config/app.toml`），
//! 以及由后端代理发起的连通性测试、目录缓存刷新与文件操作接管（完全避开浏览器 CORS 跨域限制与敏感凭证泄露）。

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
    #[serde(rename = "takeoverMode")]
    pub takeover_mode: String,
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
    #[serde(default, alias = "takeoverMode")]
    pub takeover_mode: Option<String>,
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
        takeover_mode: cfg.takeover_mode,
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

    let takeover_mode = req
        .takeover_mode
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| guard.openlist.takeover_mode.clone());

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
        takeover_mode,
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
        takeover_mode: new_openlist.takeover_mode,
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
        .timeout(Duration::from_secs(25))
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

/// 解析百度账号在 OpenList 上的挂载路径、服务地址与 root
fn resolve_account_mount(
    cfg: &OpenListConfig,
    uid: Option<u64>,
    username: Option<&str>,
) -> Result<(String, String, String), ApiError> {
    let mut mount_path = String::new();
    let mut base_url = cfg.base_url.clone();
    let mut root_path = "/".to_string();

    // 1. 尝试按 uid 查映射
    if let Some(uid_val) = uid {
        let uid_str = uid_val.to_string();
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
        if let Some(u) = username {
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
        if let Some(u) = username {
            mount_path = format!("/{}", u.trim_matches('/'));
        }
    }

    // 4. 尝试兜底挂载路径
    if mount_path.is_empty() && !cfg.fallback_mount.is_empty() {
        mount_path = cfg.fallback_mount.clone();
    }

    if mount_path.is_empty() {
        let label = username.unwrap_or("当前账号");
        return Err(ApiError::BadRequest(format!(
            "未找到账号「{}」对应的 OpenList 挂载路径，请先在设置中配置挂载点",
            label
        )));
    }

    Ok((base_url, mount_path, root_path))
}

/// 网盘完整路径转换为 OpenList 存储路径
fn to_openlist_path(mount_path: &str, root_path: &str, full_path: &str) -> Result<String, ApiError> {
    let path_clean = full_path.trim();
    let root_clean = root_path.trim().trim_end_matches('/');

    let rel_path = if root_clean.is_empty() || root_clean == "/" {
        path_clean.trim_start_matches('/')
    } else if let Some(stripped) = path_clean.strip_prefix(root_clean) {
        stripped.trim_start_matches('/')
    } else {
        return Err(ApiError::BadRequest(format!(
            "路径「{}」不在 OpenList 挂载根目录「{}」范围内",
            full_path, root_path
        )));
    };

    let mount_clean = mount_path.trim().trim_end_matches('/');
    if rel_path.is_empty() {
        Ok(mount_clean.to_string())
    } else {
        Ok(format!("{}/{}", mount_clean, rel_path))
    }
}

/// 拆分完整路径为父目录与文件名
fn split_parent_and_name(full_path: &str) -> (String, String) {
    let trimmed = full_path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(idx) => {
            let parent = if idx == 0 { "/" } else { &trimmed[..idx] };
            let name = &trimmed[idx + 1..];
            (parent.to_string(), name.to_string())
        }
        None => ("/".to_string(), trimmed.to_string()),
    }
}

/// 登录并获取 OpenList Token
async fn login_and_get_token(
    client: &reqwest::Client,
    base_url: &str,
    username: &str,
    password: &str,
) -> Result<String, ApiError> {
    let login_url = format!("{}/api/auth/login", base_url);
    let login_body = serde_json::json!({
        "username": username,
        "password": password,
    });

    let resp = client
        .post(&login_url)
        .json(&login_body)
        .send()
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("请求 OpenList 登录接口失败: {}", e)))?;

    let json = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("解析 OpenList 登录响应失败: {}", e)))?;

    if json.get("code").and_then(|c| c.as_i64()) != Some(200) {
        let msg = json
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("认证失败");
        return Err(ApiError::BadRequest(format!("OpenList 登录失败: {}", msg)));
    }

    let token = json
        .get("data")
        .and_then(|d| d.get("token"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("OpenList 未返回有效 Token")))?
        .to_string();

    Ok(token)
}

/// 带有自动重新登录与缓存刷新重试的 OpenList API 调用
async fn call_openlist_api(
    client: &reqwest::Client,
    base_url: &str,
    endpoint: &str,
    token: &mut String,
    username: &str,
    password: &str,
    body: &serde_json::Value,
    refresh_path: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let url = format!("{}{}", base_url, endpoint);

    let mut resp = client
        .post(&url)
        .header("Authorization", token.as_str())
        .json(body)
        .send()
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("请求 OpenList {} 失败: {}", endpoint, e)))?;

    let mut json = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("解析 OpenList 响应失败: {}", e)))?;

    // 401: Token 过期，重新登录重试一次
    if json.get("code").and_then(|c| c.as_i64()) == Some(401) {
        *token = login_and_get_token(client, base_url, username, password).await?;
        resp = client
            .post(&url)
            .header("Authorization", token.as_str())
            .json(body)
            .send()
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("重试 OpenList {} 失败: {}", endpoint, e)))?;
        json = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("解析重试响应失败: {}", e)))?;
    }

    // not found: 尝试刷新目录缓存后重试一次
    let code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    let msg = json.get("message").and_then(|m| m.as_str()).unwrap_or("");
    if code != 200 && msg.to_lowercase().contains("not found") {
        if let Some(rpath) = refresh_path {
            let refresh_url = format!("{}/api/fs/list", base_url);
            let refresh_body = serde_json::json!({
                "path": rpath,
                "refresh": true,
                "page": 1,
                "per_page": 1,
            });
            let _ = client
                .post(&refresh_url)
                .header("Authorization", token.as_str())
                .json(&refresh_body)
                .send()
                .await;

            resp = client
                .post(&url)
                .header("Authorization", token.as_str())
                .json(body)
                .send()
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("刷新后请求失败: {}", e)))?;
            json = resp
                .json::<serde_json::Value>()
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("解析刷新后响应失败: {}", e)))?;
        }
    }

    let final_code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    if final_code != 200 {
        let final_msg = json.get("message").and_then(|m| m.as_str()).unwrap_or("未知错误");
        if final_code == 403 || final_msg.to_lowercase().contains("permission") {
            return Err(ApiError::BadRequest(format!(
                "OpenList 用户权限不足（{}）。请在 OpenList 后台给该用户勾选对应操作权限（新建文件夹/重命名/移动/复制/删除）。",
                final_msg
            )));
        }
        return Err(ApiError::BadRequest(format!("OpenList 执行失败: {}", final_msg)));
    }

    Ok(json)
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

    let (base_url, mount_path, root_path) =
        resolve_account_mount(&cfg, req.uid, req.username.as_deref())?;

    let target_path = to_openlist_path(&mount_path, &root_path, &req.dir)?;

    info!(
        "向 OpenList 发起目录刷新: base_url={}, target_path={}",
        base_url, target_path
    );

    let client = make_http_client();
    let token = login_and_get_token(&client, &base_url, &cfg.username, &cfg.password).await?;

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

/// 文件操作条目（move / copy 单项）
#[derive(Debug, Deserialize)]
pub struct OpenListFileOpItem {
    pub path: String,
    pub dest: String,
    #[serde(default)]
    pub newname: Option<String>,
}

/// POST /openlist/execute 操作接管执行请求体
#[derive(Debug, Deserialize)]
pub struct OpenListExecuteOpRequest {
    /// 操作类型: "mkdir" | "rename" | "delete" | "move" | "copy"
    pub op: String,
    /// 账号 UID
    #[serde(default)]
    pub uid: Option<u64>,
    /// 账号用户名
    #[serde(default)]
    pub username: Option<String>,
    /// mkdir / rename 参数: 文件夹或文件完整路径
    #[serde(default)]
    pub path: Option<String>,
    /// rename 参数: 新文件名
    #[serde(default, alias = "newName", alias = "newname")]
    pub new_name: Option<String>,
    /// delete 参数: 文件完整路径列表
    #[serde(default)]
    pub paths: Option<Vec<String>>,
    /// move / copy 参数: 条目列表
    #[serde(default)]
    pub items: Option<Vec<OpenListFileOpItem>>,
}

/// 操作接管执行响应体
#[derive(Debug, Serialize)]
pub struct OpenListExecuteOpResponse {
    pub success: bool,
    pub desc: String,
    pub data: serde_json::Value,
}

/// POST /openlist/execute
/// 由后端代理向 OpenList 发起 新建文件夹 / 重命名 / 删除 / 移动 / 复制 操作接管执行
pub async fn execute_openlist_op(
    State(app_state): State<AppState>,
    Json(req): Json<OpenListExecuteOpRequest>,
) -> ApiResult<Json<ApiResponse<OpenListExecuteOpResponse>>> {
    let cfg = app_state.config.read().await.openlist.clone();

    if !cfg.enabled {
        return Err(ApiError::BadRequest("未启用 OpenList 联动".to_string()));
    }
    if cfg.base_url.is_empty() {
        return Err(ApiError::BadRequest("未配置 OpenList 服务网址".to_string()));
    }
    if cfg.username.is_empty() || cfg.password.is_empty() {
        return Err(ApiError::BadRequest(
            "未配置 OpenList 账号密码（执行接管操作需要对应写入/删除权限的专用账号）".to_string(),
        ));
    }

    let (base_url, mount_path, root_path) =
        resolve_account_mount(&cfg, req.uid, req.username.as_deref())?;

    let client = make_http_client();
    let mut token = login_and_get_token(&client, &base_url, &cfg.username, &cfg.password).await?;

    let task_id = rand::random::<u32>() as u64;

    match req.op.as_str() {
        "mkdir" => {
            let path = req.path.ok_or_else(|| ApiError::BadRequest("缺少 path 参数".to_string()))?;
            let (parent, folder_name) = split_parent_and_name(&path);

            let ol_path = to_openlist_path(&mount_path, &root_path, &path)?;
            let ol_parent = to_openlist_path(&mount_path, &root_path, &parent)?;

            let body = serde_json::json!({
                "path": ol_path,
            });

            call_openlist_api(
                &client,
                &base_url,
                "/api/fs/mkdir",
                &mut token,
                &cfg.username,
                &cfg.password,
                &body,
                Some(&ol_parent),
            )
            .await?;

            let desc = format!("新建文件夹「{}」", folder_name);

            Ok(Json(ApiResponse::success(OpenListExecuteOpResponse {
                success: true,
                desc,
                data: serde_json::json!({
                    "fs_id": task_id,
                    "path": path,
                    "isdir": 1,
                }),
            })))
        }

        "rename" => {
            let path = req.path.ok_or_else(|| ApiError::BadRequest("缺少 path 参数".to_string()))?;
            let new_name = req
                .new_name
                .ok_or_else(|| ApiError::BadRequest("缺少 new_name 参数".to_string()))?;
            let (parent, old_name) = split_parent_and_name(&path);

            let ol_path = to_openlist_path(&mount_path, &root_path, &path)?;
            let ol_parent = to_openlist_path(&mount_path, &root_path, &parent)?;

            let body = serde_json::json!({
                "path": ol_path,
                "name": new_name,
            });

            call_openlist_api(
                &client,
                &base_url,
                "/api/fs/rename",
                &mut token,
                &cfg.username,
                &cfg.password,
                &body,
                Some(&ol_parent),
            )
            .await?;

            let new_path = format!("{}/{}", parent.trim_end_matches('/'), new_name);
            let desc = format!("重命名「{}」→「{}」", old_name, new_name);

            Ok(Json(ApiResponse::success(OpenListExecuteOpResponse {
                success: true,
                desc,
                data: serde_json::json!({
                    "kind": "success",
                    "taskid": task_id,
                    "total": 1,
                    "list": [{ "from": path, "to": new_path }]
                }),
            })))
        }

        "delete" => {
            let paths = req
                .paths
                .ok_or_else(|| ApiError::BadRequest("缺少 paths 参数".to_string()))?;
            if paths.is_empty() {
                return Err(ApiError::BadRequest("删除列表不能为空".to_string()));
            }

            // 按父目录归类
            let mut groups: HashMap<String, Vec<String>> = HashMap::new();
            for p in &paths {
                let (parent, name) = split_parent_and_name(p);
                groups.entry(parent).or_default().push(name);
            }

            let mut deleted_count = 0;
            for (parent, names) in groups {
                let ol_dir = to_openlist_path(&mount_path, &root_path, &parent)?;
                let body = serde_json::json!({
                    "dir": ol_dir,
                    "names": names,
                });

                call_openlist_api(
                    &client,
                    &base_url,
                    "/api/fs/remove",
                    &mut token,
                    &cfg.username,
                    &cfg.password,
                    &body,
                    Some(&ol_dir),
                )
                .await?;

                deleted_count += names.len();
            }

            let desc = format!("删除 {} 项文件/文件夹", deleted_count);
            Ok(Json(ApiResponse::success(OpenListExecuteOpResponse {
                success: true,
                desc,
                data: serde_json::json!({
                    "deleted_count": deleted_count,
                    "failed_paths": []
                }),
            })))
        }

        "move" | "copy" => {
            let items = req
                .items
                .ok_or_else(|| ApiError::BadRequest("缺少 items 参数".to_string()))?;
            if items.is_empty() {
                return Err(ApiError::BadRequest("操作列表不能为空".to_string()));
            }

            // 检查是否有同时改名情况
            for it in &items {
                let (_, base) = split_parent_and_name(&it.path);
                if let Some(ref nn) = it.newname {
                    if nn != &base {
                        return Err(ApiError::BadRequest(
                            "OpenList 不支持在移动/复制的同时重命名，请使用原生方式执行".to_string(),
                        ));
                    }
                }
            }

            // 按 (src_parent, dest_dir) 归类
            let mut groups: HashMap<(String, String), Vec<String>> = HashMap::new();
            for it in &items {
                let (src_parent, name) = split_parent_and_name(&it.path);
                groups.entry((src_parent, it.dest.clone())).or_default().push(name);
            }

            let endpoint = if req.op == "move" {
                "/api/fs/move"
            } else {
                "/api/fs/copy"
            };

            for ((src_parent, dest_dir), names) in groups {
                let ol_src = to_openlist_path(&mount_path, &root_path, &src_parent)?;
                let ol_dst = to_openlist_path(&mount_path, &root_path, &dest_dir)?;

                let body = serde_json::json!({
                    "src_dir": ol_src,
                    "dst_dir": ol_dst,
                    "names": names,
                });

                call_openlist_api(
                    &client,
                    &base_url,
                    endpoint,
                    &mut token,
                    &cfg.username,
                    &cfg.password,
                    &body,
                    Some(&ol_src),
                )
                .await?;
            }

            let op_name = if req.op == "move" { "移动" } else { "复制" };
            let desc = format!("{} {} 项文件/文件夹", op_name, items.len());

            let list = items
                .iter()
                .map(|it| {
                    let (_, name) = split_parent_and_name(&it.path);
                    serde_json::json!({
                        "from": it.path,
                        "to": format!("{}/{}", it.dest.trim_end_matches('/'), name)
                    })
                })
                .collect::<Vec<_>>();

            Ok(Json(ApiResponse::success(OpenListExecuteOpResponse {
                success: true,
                desc,
                data: serde_json::json!({
                    "kind": "success",
                    "taskid": task_id,
                    "total": items.len(),
                    "list": list
                }),
            })))
        }

        _ => Err(ApiError::BadRequest(format!("不支持的操作类型: {}", req.op))),
    }
}
