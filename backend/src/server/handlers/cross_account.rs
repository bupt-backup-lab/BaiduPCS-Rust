// 跨账号复制 / 移动 / 下载 API 处理器

use crate::auth::Uid;
use crate::cross_account::{self, CrossAccountJob, CrossAccountMode};
use crate::server::handlers::ApiResponse;
use crate::server::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::collections::HashSet;

/// 创建跨账号复制 / 移动 / 下载任务请求
#[derive(Debug, Deserialize)]
pub struct CreateCrossAccountJobRequest {
    /// 源账号；缺省为当前活跃账号
    #[serde(default)]
    pub source_uid: Option<u64>,
    /// 目标账号（下载模式为负责下载的账号）
    pub target_uid: u64,
    /// 源账号中的文件 / 文件夹路径
    pub paths: Vec<String>,
    /// 目标账号中的保存目录（复制 / 移动必填）
    #[serde(default)]
    pub target_dir: String,
    /// 本地下载目录（下载模式必填）
    #[serde(default)]
    pub local_download_path: Option<String>,
    pub mode: CrossAccountMode,
}

/// 去掉尾部 `/`（根目录保持 `/`）
fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    let stripped = trimmed.trim_end_matches('/');
    if stripped.is_empty() {
        "/".to_string()
    } else {
        stripped.to_string()
    }
}

/// 校验请求字段并按模式建任务（账号是否存在由调用方另行校验）
fn build_job(req: &CreateCrossAccountJobRequest, source: Uid, target: Uid) -> Result<CrossAccountJob, String> {
    let paths = validate_source_paths(&req.paths)?;
    if req.mode == CrossAccountMode::Download {
        let local = req.local_download_path.as_deref().map(str::trim).unwrap_or_default();
        if local.is_empty() {
            return Err("请选择本地下载目录".to_string());
        }
        return Ok(CrossAccountJob::new_download(source, target, paths, local.to_string()));
    }
    if !req.target_dir.trim().starts_with('/') {
        return Err(format!("目标目录必须以 / 开头: {}", req.target_dir));
    }
    Ok(CrossAccountJob::new(req.mode, source, target, paths, normalize_path(&req.target_dir)))
}

/// 校验并归一化源路径
fn validate_source_paths(paths: &[String]) -> Result<Vec<String>, String> {
    if paths.is_empty() {
        return Err("请选择要操作的文件".to_string());
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(paths.len());
    for raw in paths {
        if !raw.trim().starts_with('/') {
            return Err(format!("路径必须以 / 开头: {}", raw));
        }
        let p = normalize_path(raw);
        if p == "/" {
            return Err("不能操作根目录".to_string());
        }
        if seen.insert(p.clone()) {
            normalized.push(p);
        }
    }
    // 只允许同一目录下的选中项（与网页版分享一致）。这同时排除了父子目录重叠
    // （["/d", "/d/a.txt"] 会让分享里既有整个 d 又有平级的 a.txt）和跨目录同名
    // （["/x/a.txt", "/y/a.txt"] 转存到同一目标目录必然冲突）。
    let parent = cross_account::parent_dir(&normalized[0]);
    if normalized.iter().any(|p| cross_account::parent_dir(p) != parent) {
        return Err("跨账号操作只支持同一目录下的文件，跨目录的搜索结果请分批操作".to_string());
    }
    Ok(normalized)
}

/// 创建跨账号复制 / 移动 / 下载任务（后台执行，立即返回任务快照）
///
/// POST /api/v1/cross-account/jobs
pub async fn create_cross_account_job(
    State(state): State<AppState>,
    Json(req): Json<CreateCrossAccountJobRequest>,
) -> Result<Json<ApiResponse<CrossAccountJob>>, StatusCode> {
    let source = match req.source_uid {
        Some(raw) => Uid::new(raw),
        None => match *state.active_uid.read().await {
            Some(uid) => uid,
            None => return Ok(Json(ApiResponse::error(401, "未登录".to_string()))),
        },
    };
    let target = Uid::new(req.target_uid);
    let job = match build_job(&req, source, target) {
        Ok(job) => job,
        Err(msg) => return Ok(Json(ApiResponse::error(400, msg))),
    };
    if source == target {
        let msg = match req.mode {
            CrossAccountMode::Download => "下载账号与文件所属账号相同，请直接下载",
            _ => "源账号与目标账号相同，请使用普通的复制 / 移动",
        };
        return Ok(Json(ApiResponse::error(400, msg.to_string())));
    }
    let target_label = match req.mode {
        CrossAccountMode::Download => "下载账号",
        _ => "目标账号",
    };
    for (uid, label) in [(source, "源账号"), (target, target_label)] {
        if state.client_for_or_active(Some(uid)).await.is_none() {
            return Ok(Json(ApiResponse::error(
                400,
                format!("{}（uid={}）不存在或未登录", label, uid.raw()),
            )));
        }
    }
    if state.transfer_manager_for(target).is_none() {
        return Ok(Json(ApiResponse::error(
            400,
            "目标账号转存管理器未初始化，请重新登录目标账号".to_string(),
        )));
    }

    Ok(Json(ApiResponse::success(cross_account::spawn_job(&state, job))))
}

/// 查询单个跨账号任务
///
/// GET /api/v1/cross-account/jobs/:id
pub async fn get_cross_account_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<CrossAccountJob>>, StatusCode> {
    match state.cross_account_jobs.get(&id) {
        Some(job) => Ok(Json(ApiResponse::success(job))),
        None => Ok(Json(ApiResponse::error(404, "任务不存在或已过期".to_string()))),
    }
}

/// 列出跨账号任务（含最近一小时内结束的），新的在前
///
/// GET /api/v1/cross-account/jobs
pub async fn list_cross_account_jobs(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<CrossAccountJob>>>, StatusCode> {
    Ok(Json(ApiResponse::success(state.cross_account_jobs.list())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(json: &str) -> CreateCrossAccountJobRequest {
        serde_json::from_str(json).unwrap()
    }

    fn build(json: &str) -> Result<CrossAccountJob, String> {
        build_job(&req(json), Uid::new(1), Uid::new(2))
    }

    #[test]
    fn source_paths_normalize_and_dedup() {
        let paths = validate_source_paths(&["/a/b/".into(), "/a/b".into(), "/a/c".into()]).unwrap();
        assert_eq!(paths, vec!["/a/b".to_string(), "/a/c".to_string()]);
    }

    #[test]
    fn source_paths_require_same_parent() {
        // 父子重叠
        assert!(validate_source_paths(&["/d".into(), "/d/a.txt".into()]).is_err());
        // 跨目录同名
        assert!(validate_source_paths(&["/x/a.txt".into(), "/y/a.txt".into()]).is_err());
        // 同目录多选、根目录多选
        assert!(validate_source_paths(&["/d/a".into(), "/d/b".into()]).is_ok());
        assert!(validate_source_paths(&["/a".into(), "/b".into()]).is_ok());
    }

    #[test]
    fn source_paths_reject_bad_input() {
        assert!(validate_source_paths(&[]).is_err());
        assert!(validate_source_paths(&["a/b".into()]).is_err());
        assert!(validate_source_paths(&["/".into()]).is_err());
    }

    #[test]
    fn build_copy_move_job_normalizes_target_dir() {
        let job = build(r#"{"target_uid":2,"paths":["/a"],"target_dir":"/dst/","mode":"move"}"#).unwrap();
        assert_eq!(job.mode, CrossAccountMode::Move);
        assert_eq!(job.target_dir, "/dst");
        assert!(job.local_download_path.is_none());
        let job = build(r#"{"target_uid":2,"paths":["/a"],"target_dir":"/","mode":"copy"}"#).unwrap();
        assert_eq!(job.target_dir, "/");
        // 复制 / 移动必须给出网盘目录
        assert!(build(r#"{"target_uid":2,"paths":["/a"],"target_dir":"dst","mode":"copy"}"#).is_err());
        assert!(build(r#"{"target_uid":2,"paths":["/a"],"mode":"copy"}"#).is_err());
    }

    #[test]
    fn build_download_job_requires_local_path() {
        let job = build(
            r#"{"target_uid":2,"paths":["/d/a","/d/b"],"local_download_path":" D:/dl ","mode":"download"}"#,
        )
            .unwrap();
        assert_eq!(job.mode, CrossAccountMode::Download);
        assert_eq!(job.paths, vec!["/d/a".to_string(), "/d/b".to_string()]);
        assert_eq!(job.local_download_path.as_deref(), Some("D:/dl"));
        assert_eq!(job.target_dir, "");
        assert!(build(r#"{"target_uid":2,"paths":["/a"],"mode":"download"}"#).is_err());
        assert!(build(r#"{"target_uid":2,"paths":["/a"],"local_download_path":"  ","mode":"download"}"#).is_err());
        // 源路径规则对下载同样适用
        assert!(build(r#"{"target_uid":2,"paths":["/x/a","/y/b"],"local_download_path":"D:/dl","mode":"download"}"#).is_err());
    }

    #[test]
    fn request_mode_deserializes() {
        let r = req(r#"{"target_uid":2,"paths":["/a"],"target_dir":"/","mode":"move"}"#);
        assert_eq!(r.mode, CrossAccountMode::Move);
        assert!(r.source_uid.is_none());
        assert_eq!(req(r#"{"target_uid":2,"paths":["/a"],"mode":"download"}"#).mode, CrossAccountMode::Download);
    }
}
