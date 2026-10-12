// 跨账号复制 / 移动 API 处理器

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

/// 创建跨账号复制 / 移动任务请求
#[derive(Debug, Deserialize)]
pub struct CreateCrossAccountJobRequest {
    /// 源账号；缺省为当前活跃账号
    #[serde(default)]
    pub source_uid: Option<u64>,
    pub target_uid: u64,
    /// 源账号中的文件 / 文件夹路径
    pub paths: Vec<String>,
    /// 目标账号中的保存目录
    pub target_dir: String,
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

/// 校验并归一化请求路径，返回 (paths, target_dir)
fn validate_paths(paths: &[String], target_dir: &str) -> Result<(Vec<String>, String), String> {
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
        return Err("跨账号复制/移动只支持同一目录下的文件，跨目录的搜索结果请分批操作".to_string());
    }
    if !target_dir.trim().starts_with('/') {
        return Err(format!("目标目录必须以 / 开头: {}", target_dir));
    }
    Ok((normalized, normalize_path(target_dir)))
}

/// 创建跨账号复制 / 移动任务（后台执行，立即返回任务快照）
///
/// POST /api/v1/cross-account/jobs
pub async fn create_cross_account_job(
    State(state): State<AppState>,
    Json(req): Json<CreateCrossAccountJobRequest>,
) -> Result<Json<ApiResponse<CrossAccountJob>>, StatusCode> {
    let (paths, target_dir) = match validate_paths(&req.paths, &req.target_dir) {
        Ok(v) => v,
        Err(msg) => return Ok(Json(ApiResponse::error(400, msg))),
    };

    let source = match req.source_uid {
        Some(raw) => Uid::new(raw),
        None => match *state.active_uid.read().await {
            Some(uid) => uid,
            None => return Ok(Json(ApiResponse::error(401, "未登录".to_string()))),
        },
    };
    let target = Uid::new(req.target_uid);
    if source == target {
        return Ok(Json(ApiResponse::error(
            400,
            "源账号与目标账号相同，请使用普通的复制 / 移动".to_string(),
        )));
    }
    for (uid, label) in [(source, "源账号"), (target, "目标账号")] {
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

    let job = CrossAccountJob::new(req.mode, source, target, paths, target_dir);
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

    #[test]
    fn validate_paths_normalizes_and_dedups() {
        let (paths, dir) =
            validate_paths(&["/a/b/".into(), "/a/b".into(), "/a/c".into()], "/dst/").unwrap();
        assert_eq!(paths, vec!["/a/b".to_string(), "/a/c".to_string()]);
        assert_eq!(dir, "/dst");
        let (_, root) = validate_paths(&["/a".into()], "/").unwrap();
        assert_eq!(root, "/");
    }

    #[test]
    fn validate_paths_requires_same_parent() {
        // 父子重叠
        assert!(validate_paths(&["/d".into(), "/d/a.txt".into()], "/").is_err());
        // 跨目录同名
        assert!(validate_paths(&["/x/a.txt".into(), "/y/a.txt".into()], "/").is_err());
        // 同目录多选、根目录多选
        assert!(validate_paths(&["/d/a".into(), "/d/b".into()], "/").is_ok());
        assert!(validate_paths(&["/a".into(), "/b".into()], "/").is_ok());
    }

    #[test]
    fn validate_paths_rejects_bad_input() {
        assert!(validate_paths(&[], "/").is_err());
        assert!(validate_paths(&["a/b".into()], "/").is_err());
        assert!(validate_paths(&["/".into()], "/").is_err());
        assert!(validate_paths(&["/a".into()], "dst").is_err());
    }

    #[test]
    fn request_mode_deserializes() {
        let req: CreateCrossAccountJobRequest = serde_json::from_str(
            r#"{"target_uid":2,"paths":["/a"],"target_dir":"/","mode":"move"}"#,
        )
        .unwrap();
        assert_eq!(req.mode, CrossAccountMode::Move);
        assert!(req.source_uid.is_none());
    }
}
