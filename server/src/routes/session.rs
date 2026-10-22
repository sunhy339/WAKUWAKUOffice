use axum::extract::Path;
use axum::extract::Query;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::middleware::AuthUser;
use crate::db::{project_repo, session_repo};
use crate::error::AppError;
use crate::models::Artifact;
use crate::state;

pub fn router() -> Router {
    Router::new()
        .route("/api/chat/sessions", get(list_sessions))
        .route(
            "/api/chat/session/:session_id",
            get(get_session)
                .patch(update_session)
                .delete(delete_session),
        )
        .route(
            "/api/chat/session/:session_id/artifacts",
            get(get_session_artifacts).put(save_session_artifacts),
        )
        .route(
            "/api/chat/session/:session_id/artifacts/:artifact_id/versions",
            get(list_artifact_versions),
        )
        .route(
            "/api/chat/session/:session_id/artifacts/:artifact_id/versions/:version/restore",
            post(restore_artifact_version),
        )
        .route("/api/chat/session/:session_id/messages", get(get_messages))
        .route("/api/chat/session/:session_id/clear", post(clear_session))
}

#[derive(Deserialize)]
struct SessionListQuery {
    q: Option<String>,
}

#[derive(Deserialize, Default)]
struct SessionDetailQuery {
    include_artifacts: Option<bool>,
}

#[derive(Deserialize)]
struct UpdateSessionPayload {
    title: Option<String>,
    project_id: Option<Value>,
    order_col: Option<i64>,
}

#[derive(Deserialize)]
struct SaveArtifactsPayload {
    artifacts: Vec<Artifact>,
}

async fn list_sessions(
    user: AuthUser,
    Query(query): Query<SessionListQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = state::db_pool();
    tracing::info!(
        "[Sessions] list_sessions called: user_id={}, username={}, query={:?}",
        user.0.id,
        user.0.username,
        query.q
    );
    let sessions = session_repo::list_by_owner(&pool, &user.0.id, 50, query.q.as_deref()).await?;
    tracing::info!(
        "[Sessions] list_sessions: user={}, count={}",
        user.0.id,
        sessions.len()
    );
    if let Some(first) = sessions.first() {
        tracing::info!(
            "[Sessions] first session: id={}, title={}",
            first.id,
            first.title
        );
    }
    let json_val = serde_json::to_value(&sessions).map_err(|e| {
        tracing::error!("[Sessions] serialize error: {:?}", e);
        AppError::Internal(anyhow::anyhow!("serialize error: {}", e))
    })?;
    tracing::info!(
        "[Sessions] serialized OK, len={}",
        json_val.as_array().map(|a| a.len()).unwrap_or(0)
    );
    Ok(Json(json!({ "sessions": json_val })))
}

async fn get_session(
    user: AuthUser,
    Path(session_id): Path<String>,
    Query(query): Query<SessionDetailQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = state::db_pool();
    tracing::info!(
        "[Session] get_session called: session_id={}, user={}",
        session_id,
        user.0.id
    );
    let session = session_repo::get_session_detail(
        &pool,
        &session_id,
        query.include_artifacts.unwrap_or(false),
    )
    .await
    .map_err(|e| {
        tracing::error!("[Session] get_session_detail error: {:?}", e);
        e
    })?
    .ok_or(AppError::NotFound("会话不存在".into()))?;
    if session.owner_id != user.0.id {
        return Err(AppError::Forbidden);
    }
    Ok(Json(json!(session)))
}

async fn get_session_artifacts(
    user: AuthUser,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = state::db_pool();
    let session = session_repo::find_by_id(&pool, &session_id)
        .await?
        .ok_or(AppError::NotFound("会话不存在".into()))?;
    if session.owner_id != user.0.id {
        return Err(AppError::Forbidden);
    }
    let artifacts = session_repo::get_artifacts(&pool, &session_id).await?;
    Ok(Json(json!({ "artifacts": artifacts })))
}

async fn save_session_artifacts(
    user: AuthUser,
    Path(session_id): Path<String>,
    Json(payload): Json<SaveArtifactsPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = state::db_pool();
    let session = session_repo::find_by_id(&pool, &session_id)
        .await?
        .ok_or(AppError::NotFound("会话不存在".into()))?;
    if session.owner_id != user.0.id {
        return Err(AppError::Forbidden);
    }

    session_repo::save_artifacts_with_versions(
        &pool,
        &session_id,
        &payload.artifacts,
        Some(&user.0.id),
    )
    .await?;
    Ok(Json(
        json!({ "saved": true, "artifacts": payload.artifacts }),
    ))
}

async fn list_artifact_versions(
    user: AuthUser,
    Path((session_id, artifact_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = state::db_pool();
    let session = session_repo::find_by_id(&pool, &session_id)
        .await?
        .ok_or(AppError::NotFound("会话不存在".into()))?;
    if session.owner_id != user.0.id {
        return Err(AppError::Forbidden);
    }

    // 兼容版本管理上线前已经存在的会话：第一次打开版本面板时，
    // 将当前快照作为初始版本写入，后续仍然只按内容变化创建新版本。
    let artifacts = session_repo::get_artifacts(&pool, &session_id).await?;
    if artifacts.iter().any(|artifact| artifact.id == artifact_id) {
        session_repo::save_artifacts_with_versions(&pool, &session_id, &artifacts, None).await?;
    } else {
        return Err(AppError::NotFound("产物不存在".into()));
    }

    let versions = session_repo::get_artifact_versions(&pool, &session_id, &artifact_id).await?;
    Ok(Json(json!({
        "artifact_id": artifact_id,
        "versions": versions,
    })))
}

async fn restore_artifact_version(
    user: AuthUser,
    Path((session_id, artifact_id, version)): Path<(String, String, i64)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = state::db_pool();
    let session = session_repo::find_by_id(&pool, &session_id)
        .await?
        .ok_or(AppError::NotFound("会话不存在".into()))?;
    if session.owner_id != user.0.id {
        return Err(AppError::Forbidden);
    }

    let artifact = session_repo::restore_artifact_version(
        &pool,
        &session_id,
        &artifact_id,
        version,
        Some(&user.0.id),
    )
    .await?
    .ok_or(AppError::NotFound("产物版本不存在".into()))?;

    // PPT 的预览项目使用独立 JSON 文件作为渲染来源，恢复会话快照时也要
    // 同步该文件，否则刷新页面后项目文件会把恢复结果覆盖回去。
    if artifact.kind == "ppt" {
        if let Some(project_id) = artifact
            .content
            .get("project_id")
            .and_then(|value| value.as_str())
        {
            if let Some(mut project) = project_repo::load_ppt_project(project_id)? {
                if project.owner_id != user.0.id {
                    return Err(AppError::Forbidden);
                }
                if let Some(slides) = artifact.content.get("slides") {
                    project.slides = serde_json::from_value(slides.clone()).map_err(|error| {
                        AppError::BadRequest(format!("PPT 版本内容无效: {error}"))
                    })?;
                }
                if let Some(theme) = artifact
                    .content
                    .get("theme")
                    .and_then(|value| value.as_str())
                {
                    project.theme = theme.to_string();
                }
                if let Some(history) = artifact.content.get("history") {
                    project.history = serde_json::from_value(history.clone()).ok();
                }
                project.updated_at = artifact.updated_at.clone();
                project_repo::save_ppt_project(&project)?;
            }
        }
    }

    Ok(Json(json!({
        "restored": true,
        "artifact": artifact,
    })))
}

async fn get_messages(
    user: AuthUser,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = state::db_pool();
    let session = session_repo::find_by_id(&pool, &session_id)
        .await?
        .ok_or(AppError::NotFound("会话不存在".into()))?;
    if session.owner_id != user.0.id {
        return Err(AppError::Forbidden);
    }
    let messages = session_repo::get_messages(&pool, &session_id, 100).await?;
    Ok(Json(json!({ "messages": messages })))
}

async fn update_session(
    user: AuthUser,
    Path(session_id): Path<String>,
    Json(payload): Json<UpdateSessionPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = state::db_pool();
    let mut updated = false;

    if let Some(title) = payload.title {
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err(AppError::BadRequest("标题不能为空".into()));
        }
        updated |= session_repo::update_title(&pool, &session_id, &user.0.id, &title).await?;
    }

    if payload.project_id.is_some() || payload.order_col.is_some() {
        let session = session_repo::find_by_id(&pool, &session_id)
            .await?
            .ok_or(AppError::NotFound("会话不存在".into()))?;
        if session.owner_id != user.0.id {
            return Err(AppError::Forbidden);
        }
        let project_id_owned = match payload.project_id {
            Some(Value::Null) => None,
            Some(Value::String(value)) => {
                Some(value.trim().to_string()).filter(|value| !value.is_empty())
            }
            Some(_) => return Err(AppError::BadRequest("项目 ID 格式不正确".into())),
            None => session.project_id,
        };
        let order_col = payload.order_col.unwrap_or(session.order_col);
        updated |= session_repo::update_project_and_order(
            &pool,
            &session_id,
            &user.0.id,
            project_id_owned.as_deref(),
            order_col,
        )
        .await?;
    }

    Ok(Json(json!({ "updated": updated })))
}

async fn delete_session(
    user: AuthUser,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = state::db_pool();
    let deleted = session_repo::delete(&pool, &session_id, &user.0.id).await?;
    Ok(Json(json!({ "deleted": deleted })))
}

async fn clear_session(
    user: AuthUser,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = state::db_pool();
    let cleared = session_repo::clear_messages(&pool, &session_id, &user.0.id).await?;
    Ok(Json(json!({ "cleared": cleared })))
}
