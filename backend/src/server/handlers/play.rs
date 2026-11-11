use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
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
        map.retain(|_, (t, _)| t.elapsed() < CACHE_TTL);
        map.insert(path, (Instant::now(), url));
    }
}

pub async fn play(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((token, path)): Path<(String, String)>,
) -> Response {
    let expect = match std::env::var("PLAY_TOKEN") {
        Ok(t) if t.len() >= 16 => t,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    if token != expect {
        return StatusCode::NOT_FOUND.into_response();
    }

    let path = format!("/{}", path.trim_start_matches('/'));

    let dlink = if let Some(url) = cache_get(&path) {
        url
    } else {
        let client = match state.active_client().await {
            Some(c) => c,
            None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        };

        match client.get_download_url(&path, 0).await {
            Ok(url) => {
                info!("play: 获取直链成功 path={}", path);
                cache_put(path.clone(), url.clone());
                url
            }
            Err(e) => {
                error!("play: 获取直链失败 path={}, err={}", path, e);
                return StatusCode::BAD_GATEWAY.into_response();
            }
        }
    };

    let client = reqwest::Client::new();
    // 修复点1：全换成纯字符串传参，避开版本冲突
    let mut req = client.get(&dlink)
        .header("User-Agent", "netdisk;P2SP;3.0.0.8;netdisk;11.12.3;ANG-AN00;android-android;10.0;JSbridge4.4.0;jointBridge;1.1.0;");

    // 修复点2：取出 axum 的值后，解析成纯字符串再给 reqwest
    if let Some(range) = headers.get(header::RANGE) {
        if let Ok(range_str) = range.to_str() {
            req = req.header("Range", range_str);
        }
    }

    match req.send().await {
        Ok(res) => {
            // 修复点3：把 reqwest 的状态码转成数字字面量
            let status_code = res.status().as_u16();
            let mut builder = Response::builder().status(status_code);
            
            // 修复点4：把 reqwest 的头转成纯字符串，再塞回给 axum
            for (k, v) in res.headers() {
                let k_str = k.as_str();
                if k_str == "content-type" 
                    || k_str == "content-length" 
                    || k_str == "content-range" 
                    || k_str == "accept-ranges" 
                {
                    if let Ok(v_str) = v.to_str() {
                        builder = builder.header(k_str, v_str);
                    }
                }
            }

            let stream = res.bytes_stream();
            
            // 兼容写法：如果是较新版本的 axum
            let body = axum::body::Body::from_stream(stream);
            
            builder.body(body).unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(e) => {
            error!("play: VPS反代拉流失败 err={}", e);
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
