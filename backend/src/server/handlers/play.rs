use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use axum::{
    body::Body,
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
    headers: HeaderMap, // 接收 mpv 播放器发来的请求头
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

    // 1. 获取百度真实直链
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

    // 2. VPS 发起请求，伪装成百度网盘客户端
    let client = reqwest::Client::new();
    let mut req = client.get(&dlink)
        .header(header::USER_AGENT, "netdisk;P2SP;3.0.0.8;netdisk;11.12.3;ANG-AN00;android-android;10.0;JSbridge4.4.0;jointBridge;1.1.0;");

    // 【关键】：如果播放器拖拽了进度条，会发来 Range 头，我们需要把它转交给百度
    if let Some(range) = headers.get(header::RANGE) {
        req = req.header(header::RANGE, range);
    }

    // 3. 将百度的视频流直接“管线传输”给播放器
    match req.send().await {
        Ok(res) => {
            let status = res.status();
            let mut builder = Response::builder().status(status);
            
            // 把百度的响应头透传回 mpv，告诉 mpv 视频有多大、支持拖拽
            for (k, v) in res.headers() {
                if k == header::CONTENT_TYPE 
                    || k == header::CONTENT_LENGTH 
                    || k == header::CONTENT_RANGE 
                    || k == header::ACCEPT_RANGES 
                {
                    builder = builder.header(k.clone(), v.clone());
                }
            }

            // 获取底层的字节流
            let stream = res.bytes_stream();
            
            // 注意：BaiduPCS-Rust 目前应该使用的是 Axum 0.7
            let body = Body::from_stream(stream); 
            // ⚠️ 如果你在 GitHub Actions 编译时看到报错找不到 `from_stream`，
            // 请把上面这句删掉，换成： let body = Body::wrap_stream(stream);

            builder.body(body).unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(e) => {
            error!("play: VPS反代拉流失败 err={}", e);
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
