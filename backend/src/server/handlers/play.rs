//! 免登录播放地址: GET /play/:token/*path  ->  302 跳转到百度直链
//! token 取自环境变量 PLAY_TOKEN（至少 16 位），未设置则整个功能关闭。
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use tracing::{error, info};

use crate::server::AppState;

const CACHE_TTL: Duration = Duration::from_secs(600);
static CACHE: OnceLock<Mutex<HashMap<String, (Instant, String)>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, (Instant, String)>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_get(path: &str) -> Option<String> {
    let map = cache().lock().ok()?;
    let (t, url) = map.get(path)?;
    (t.elapsed() < CACHE_TTL).then(|| url.clone())
}

fn cache_put(path: String, url: String) {
    if let Ok(mut map) = cache().lock() {
        map.retain(|_, (t, _)| t.elapsed() < CACHE_TTL); // 清理过期项
        map.insert(path, (Instant::now(), url));
    }
}

pub async fn play(
    State(state): State<AppState>,
    Path((token, path)): Path<(String, String)>,
) -> Response {
    // 相当于 System.getenv("PLAY_TOKEN")
    let expect = match std::env::var("PLAY_TOKEN") {
        Ok(t) if t.len() >= 16 => t,
        _ => return StatusCode::NOT_FOUND.into_response(), // 没配环境变量直接 404
    };
    if token != expect {
        return StatusCode::NOT_FOUND.into_response(); // 鉴权失败
    }

    let path = format!("/{}", path.trim_start_matches('/'));

    if let Some(url) = cache_get(&path) {
        // 命中缓存，直接 302 跳转
        return (StatusCode::FOUND, [(header::LOCATION, url)]).into_response();
    }

    let client = match state.active_client().await {
        Some(c) => c,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    // 调底层方法获取直链
    match client.get_download_url(&path, 0).await {
        Ok(url) => {
            info!("play: 获取直链成功 path={}", path);
            cache_put(path.clone(), url.clone());
            // 返回 302 重定向
            (StatusCode::FOUND, [(header::LOCATION, url)]).into_response()
        }
        Err(e) => {
            error!("play: 获取直链失败 path={}, err={}", path, e);
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
