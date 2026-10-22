use super::DbPool;
use crate::error::AppResult;
use crate::models::{Artifact, ChatMessage, PersistedChatMessage, SessionProcessEvent};
use serde::{Deserialize, Serialize};
use sqlx::any::AnyRow;
use sqlx::Row;

/// Read text fields while remaining compatible with data written by older
/// versions as BLOB values.
fn read_text(row: &AnyRow, index: usize) -> Result<String, sqlx::Error> {
    match row.try_get::<String, _>(index) {
        Ok(value) => Ok(value),
        Err(text_error) => row
            .try_get::<Vec<u8>, _>(index)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .map_err(|_| text_error),
    }
}

fn read_optional_text(row: &AnyRow, index: usize) -> Result<Option<String>, sqlx::Error> {
    match row.try_get::<Option<String>, _>(index) {
        Ok(value) => Ok(value),
        Err(text_error) => row
            .try_get::<Option<Vec<u8>>, _>(index)
            .map(|value| value.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
            .map_err(|_| text_error),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRow {
    pub id: String,
    pub owner_id: String,
    pub project_id: Option<String>,
    pub tool_kind: Option<String>,
    pub title: String,
    pub summary: Option<String>,
    pub message_count: i64,
    pub order_col: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetail {
    pub id: String,
    pub owner_id: String,
    pub project_id: Option<String>,
    pub tool_kind: Option<String>,
    pub title: String,
    pub summary: Option<String>,
    pub process_events: Vec<SessionProcessEvent>,
    pub message_count: i64,
    pub order_col: i64,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<PersistedChatMessage>,
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactVersion {
    pub id: String,
    pub session_id: String,
    pub artifact_id: String,
    pub version: i64,
    pub artifact: Artifact,
    pub created_at: String,
    pub created_by: Option<String>,
}

pub async fn create(
    pool: &DbPool,
    owner_id: &str,
    project_id: Option<&str>,
    tool_kind: Option<&str>,
    title: &str,
) -> AppResult<SessionRow> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let order_val = chrono::Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO sessions (id, owner_id, project_id, tool_kind, title, message_count, order_col, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?)"
    )
    .bind(&id)
    .bind(owner_id)
    .bind(project_id)
    .bind(tool_kind)
    .bind(title)
    .bind(order_val)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(SessionRow {
        id,
        owner_id: owner_id.to_string(),
        project_id: project_id.map(String::from),
        tool_kind: tool_kind.map(String::from),
        title: title.to_string(),
        summary: None,
        message_count: 0,
        order_col: order_val,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn find_by_id(pool: &DbPool, id: &str) -> AppResult<Option<SessionRow>> {
    let row = sqlx::query(
        "SELECT id, owner_id, project_id, tool_kind, title, CAST(summary AS CHAR) AS summary, message_count, order_col, created_at, updated_at
         FROM sessions WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(Some(SessionRow {
            id: r.try_get(0).map_err(|e| {
                tracing::error!("[Session] find_by_id col 0 (id) error: {:?}", e);
                e
            })?,
            owner_id: r.try_get(1).map_err(|e| {
                tracing::error!("[Session] find_by_id col 1 (owner_id) error: {:?}", e);
                e
            })?,
            project_id: r.try_get(2).map_err(|e| {
                tracing::error!("[Session] find_by_id col 2 (project_id) error: {:?}", e);
                e
            })?,
            tool_kind: r.try_get(3).map_err(|e| {
                tracing::error!("[Session] find_by_id col 3 (tool_kind) error: {:?}", e);
                e
            })?,
            title: r.try_get(4).map_err(|e| {
                tracing::error!("[Session] find_by_id col 4 (title) error: {:?}", e);
                e
            })?,
            summary: r.try_get(5).map_err(|e| {
                tracing::error!("[Session] find_by_id col 5 (summary) error: {:?}", e);
                e
            })?,
            message_count: r.try_get(6).map_err(|e| {
                tracing::error!("[Session] find_by_id col 6 (message_count) error: {:?}", e);
                e
            })?,
            order_col: r.try_get(7).map_err(|e| {
                tracing::error!("[Session] find_by_id col 7 (order_col) error: {:?}", e);
                e
            })?,
            created_at: r.try_get(8).map_err(|e| {
                tracing::error!("[Session] find_by_id col 8 (created_at) error: {:?}", e);
                e
            })?,
            updated_at: r.try_get(9).map_err(|e| {
                tracing::error!("[Session] find_by_id col 9 (updated_at) error: {:?}", e);
                e
            })?,
        })),
        None => Ok(None),
    }
}

pub async fn list_by_owner(
    pool: &DbPool,
    owner_id: &str,
    limit: i64,
    query: Option<&str>,
) -> AppResult<Vec<SessionRow>> {
    let q = query
        .map(|item| format!("%{}%", item.trim()))
        .filter(|item| item != "%%");

    tracing::info!(
        "[Sessions] list_by_owner: owner_id={}, limit={}, has_query={}",
        owner_id,
        limit,
        q.is_some()
    );

    let rows = if let Some(ref qv) = q {
        sqlx::query(
            "SELECT id, owner_id, project_id, tool_kind, title, CAST(summary AS CHAR) AS summary, message_count, order_col, created_at, updated_at
             FROM sessions
             WHERE owner_id = ? AND (title LIKE ? OR COALESCE(summary, '') LIKE ?)
             ORDER BY order_col ASC, updated_at DESC LIMIT ?"
        )
        .bind(owner_id)
        .bind(qv)
        .bind(qv)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, owner_id, project_id, tool_kind, title, CAST(summary AS CHAR) AS summary, message_count, order_col, created_at, updated_at
             FROM sessions WHERE owner_id = ? ORDER BY order_col ASC, updated_at DESC LIMIT ?"
        )
        .bind(owner_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    tracing::info!("[Sessions] list_by_owner: fetched {} rows", rows.len());
    if !rows.is_empty() {
        let first_id: String = rows[0].try_get(0).unwrap_or_default();
        let first_title: String = rows[0].try_get(4).unwrap_or_default();
        tracing::info!(
            "[Sessions] first row: id={}, title={}",
            first_id,
            first_title
        );
    }

    let mut result = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        result.push(SessionRow {
            id: row.try_get(0).map_err(|e| {
                tracing::error!("[Sessions] row {} col 0 (id) error: {:?}", i, e);
                e
            })?,
            owner_id: row.try_get(1).map_err(|e| {
                tracing::error!("[Sessions] row {} col 1 (owner_id) error: {:?}", i, e);
                e
            })?,
            project_id: row.try_get(2).map_err(|e| {
                tracing::error!("[Sessions] row {} col 2 (project_id) error: {:?}", i, e);
                e
            })?,
            tool_kind: row.try_get(3).map_err(|e| {
                tracing::error!("[Sessions] row {} col 3 (tool_kind) error: {:?}", i, e);
                e
            })?,
            title: row.try_get(4).map_err(|e| {
                tracing::error!("[Sessions] row {} col 4 (title) error: {:?}", i, e);
                e
            })?,
            summary: row.try_get(5).map_err(|e| {
                tracing::error!("[Sessions] row {} col 5 (summary) error: {:?}", i, e);
                e
            })?,
            message_count: row.try_get(6).map_err(|e| {
                tracing::error!("[Sessions] row {} col 6 (message_count) error: {:?}", i, e);
                e
            })?,
            order_col: row.try_get(7).map_err(|e| {
                tracing::error!("[Sessions] row {} col 7 (order_col) error: {:?}", i, e);
                e
            })?,
            created_at: row.try_get(8).map_err(|e| {
                tracing::error!("[Sessions] row {} col 8 (created_at) error: {:?}", i, e);
                e
            })?,
            updated_at: row.try_get(9).map_err(|e| {
                tracing::error!("[Sessions] row {} col 9 (updated_at) error: {:?}", i, e);
                e
            })?,
        });
    }
    Ok(result)
}

pub async fn update_summary(pool: &DbPool, session_id: &str, summary: &str) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE sessions SET summary = ?, updated_at = ? WHERE id = ?")
        .bind(summary)
        .bind(&now)
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_title(
    pool: &DbPool,
    session_id: &str,
    owner_id: &str,
    title: &str,
) -> AppResult<bool> {
    let now = chrono::Utc::now().to_rfc3339();
    let result =
        sqlx::query("UPDATE sessions SET title = ?, updated_at = ? WHERE id = ? AND owner_id = ?")
            .bind(title)
            .bind(&now)
            .bind(session_id)
            .bind(owner_id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_project_and_order(
    pool: &DbPool,
    session_id: &str,
    owner_id: &str,
    project_id: Option<&str>,
    order_col: i64,
) -> AppResult<bool> {
    let result = sqlx::query(
        "UPDATE sessions SET project_id = ?, order_col = ? WHERE id = ? AND owner_id = ?",
    )
    .bind(project_id)
    .bind(order_col)
    .bind(session_id)
    .bind(owner_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn save_artifacts(
    pool: &DbPool,
    session_id: &str,
    artifacts: &[Artifact],
) -> AppResult<()> {
    save_artifacts_with_versions(pool, session_id, artifacts, None).await
}

pub async fn save_artifacts_with_versions(
    pool: &DbPool,
    session_id: &str,
    artifacts: &[Artifact],
    created_by: Option<&str>,
) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::to_string(artifacts)?;
    let id = uuid::Uuid::new_v4().to_string();
    let created_by = created_by.map(str::to_owned);

    let mut tx = pool.begin().await?;

    // 使用 UPSERT：SQLite 用 ON CONFLICT，MySQL 用 ON DUPLICATE KEY UPDATE
    let cfg = crate::config::config();
    if cfg.is_mysql() {
        sqlx::query(
            "INSERT INTO session_artifacts (id, session_id, payload, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON DUPLICATE KEY UPDATE payload = VALUES(payload), updated_at = VALUES(updated_at)",
        )
        .bind(&id)
        .bind(session_id)
        .bind(&payload)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO session_artifacts (id, session_id, payload, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(session_id) DO UPDATE SET payload = excluded.payload, updated_at = excluded.updated_at"
        )
        .bind(&id)
        .bind(session_id)
        .bind(&payload)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }

    for artifact in artifacts {
        let artifact_payload = serde_json::to_string(artifact)?;
        let latest = sqlx::query(
            "SELECT payload, version
             FROM artifact_versions
             WHERE session_id = ? AND artifact_id = ?
             ORDER BY version DESC LIMIT 1",
        )
        .bind(session_id)
        .bind(&artifact.id)
        .fetch_optional(&mut *tx)
        .await?;

        let (is_unchanged, next_version) = match latest {
            Some(row) => {
                let latest_payload = read_text(&row, 0)?;
                let latest_version: i64 = row.try_get(1)?;
                (
                    latest_payload == artifact_payload,
                    latest_version.saturating_add(1),
                )
            }
            None => (false, 1),
        };

        if !is_unchanged {
            sqlx::query(
                "INSERT INTO artifact_versions
                 (id, session_id, artifact_id, version, payload, created_at, created_by)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(session_id)
            .bind(&artifact.id)
            .bind(next_version)
            .bind(&artifact_payload)
            .bind(&now)
            .bind(&created_by)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

pub async fn get_artifact_versions(
    pool: &DbPool,
    session_id: &str,
    artifact_id: &str,
) -> AppResult<Vec<ArtifactVersion>> {
    let rows = sqlx::query(
        "SELECT id, version, payload, created_at, created_by
         FROM artifact_versions
         WHERE session_id = ? AND artifact_id = ?
         ORDER BY version DESC",
    )
    .bind(session_id)
    .bind(artifact_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let payload = read_text(&row, 2)?;
            let artifact: Artifact = serde_json::from_str(&payload)?;
            Ok(ArtifactVersion {
                id: read_text(&row, 0)?,
                session_id: session_id.to_string(),
                artifact_id: artifact_id.to_string(),
                version: row.try_get(1)?,
                artifact,
                created_at: row.try_get(3)?,
                created_by: read_optional_text(&row, 4)?,
            })
        })
        .collect()
}

pub async fn restore_artifact_version(
    pool: &DbPool,
    session_id: &str,
    artifact_id: &str,
    version: i64,
    created_by: Option<&str>,
) -> AppResult<Option<Artifact>> {
    let row = sqlx::query(
        "SELECT payload FROM artifact_versions
         WHERE session_id = ? AND artifact_id = ? AND version = ?",
    )
    .bind(session_id)
    .bind(artifact_id)
    .bind(version)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let payload = read_text(&row, 0)?;
    let mut restored: Artifact = serde_json::from_str(&payload)?;
    let mut artifacts = get_artifacts(pool, session_id).await?;
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(index) = artifacts
        .iter()
        .position(|artifact| artifact.id == artifact_id)
    {
        let current = &artifacts[index];
        restored.id = current.id.clone();
        restored.created_at = current.created_at.clone();
        restored.version = current.version.saturating_add(1);
        restored.updated_at = now;
        artifacts[index] = restored.clone();
    } else {
        restored.updated_at = now;
        artifacts.push(restored.clone());
    }

    save_artifacts_with_versions(pool, session_id, &artifacts, created_by).await?;
    Ok(Some(restored))
}

pub async fn get_artifacts(pool: &DbPool, session_id: &str) -> AppResult<Vec<Artifact>> {
    let row = sqlx::query("SELECT payload FROM session_artifacts WHERE session_id = ?")
        .bind(session_id)
        .fetch_optional(pool)
        .await?;

    match row {
        Some(r) => {
            let payload = read_text(&r, 0).map_err(|e| {
                tracing::error!("[Artifacts] col 0 (payload) error: {:?}", e);
                e
            })?;
            Ok(serde_json::from_str::<Vec<Artifact>>(&payload)?)
        }
        None => Ok(vec![]),
    }
}

pub async fn get_session_detail(
    pool: &DbPool,
    session_id: &str,
    include_artifacts: bool,
) -> AppResult<Option<SessionDetail>> {
    let session = match find_by_id(pool, session_id).await? {
        Some(session) => session,
        None => return Ok(None),
    };
    let messages = get_persisted_messages(pool, session_id, 100).await?;
    // Image/video artifacts may contain large base64 payloads. Keep them out of
    // the fast history response so messages and process events can render first.
    let artifacts = if include_artifacts {
        get_artifacts(pool, session_id).await?
    } else {
        Vec::new()
    };
    let process_events = get_process_events(pool, session_id).await?;
    Ok(Some(SessionDetail {
        id: session.id,
        owner_id: session.owner_id,
        project_id: session.project_id,
        tool_kind: session.tool_kind,
        title: session.title,
        summary: session.summary,
        process_events,
        message_count: session.message_count,
        order_col: session.order_col,
        created_at: session.created_at,
        updated_at: session.updated_at,
        messages,
        artifacts,
    }))
}

pub async fn get_process_events(
    pool: &DbPool,
    session_id: &str,
) -> AppResult<Vec<SessionProcessEvent>> {
    let row = sqlx::query("SELECT process_events FROM sessions WHERE id = ?")
        .bind(session_id)
        .fetch_optional(pool)
        .await?;

    let Some(row) = row else {
        return Ok(Vec::new());
    };

    let payload = read_optional_text(&row, 0)?.unwrap_or_else(|| "[]".to_string());
    match serde_json::from_str::<Vec<SessionProcessEvent>>(&payload) {
        Ok(events) => Ok(events),
        Err(error) => {
            tracing::warn!(
                "[Sessions] invalid process_events for {}: {}",
                session_id,
                error
            );
            Ok(Vec::new())
        }
    }
}

pub async fn save_process_events(
    pool: &DbPool,
    session_id: &str,
    events: &[SessionProcessEvent],
) -> AppResult<()> {
    let payload = serde_json::to_string(events)?;
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE sessions SET process_events = ?, updated_at = ? WHERE id = ?")
        .bind(&payload)
        .bind(&now)
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn add_message(pool: &DbPool, session_id: &str, msg: &ChatMessage) -> AppResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let tool_name: Option<String> = None;
    let tool_input: Option<String> = msg
        .tool_calls
        .as_ref()
        .map(|t| serde_json::to_string(t).unwrap_or_default());
    let tool_output: Option<String> = msg.tool_call_id.clone();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, tool_name, tool_input, tool_output, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(session_id)
    .bind(&msg.role)
    .bind(&msg.content)
    .bind(&tool_name)
    .bind(&tool_input)
    .bind(&tool_output)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE sessions SET message_count = message_count + 1, updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_messages(
    pool: &DbPool,
    session_id: &str,
    limit: i64,
) -> AppResult<Vec<ChatMessage>> {
    let rows = sqlx::query(
        "SELECT role, content, tool_input, tool_output FROM messages
         WHERE session_id = ? ORDER BY created_at ASC LIMIT ?",
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        let role: String = row.try_get(0)?;
        let content = read_text(&row, 1)?;
        let tool_input = read_optional_text(&row, 2)?;
        let tool_output = read_optional_text(&row, 3)?;
        let tool_calls = tool_input
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());
        let tool_call_id = tool_output;
        result.push(ChatMessage {
            role,
            content,
            tool_calls,
            tool_call_id,
        });
    }
    Ok(result)
}

pub async fn get_persisted_messages(
    pool: &DbPool,
    session_id: &str,
    limit: i64,
) -> AppResult<Vec<PersistedChatMessage>> {
    let rows = sqlx::query(
        "SELECT role, content, tool_input, tool_output, created_at FROM messages
         WHERE session_id = ? ORDER BY created_at ASC LIMIT ?",
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let role: String = row.try_get(0).map_err(|e| {
            tracing::error!("[Messages] row {} col 0 (role) error: {:?}", i, e);
            e
        })?;
        let content = read_text(row, 1).map_err(|e| {
            tracing::error!("[Messages] row {} col 1 (content) error: {:?}", i, e);
            e
        })?;
        let tool_input = read_optional_text(row, 2).map_err(|e| {
            tracing::error!("[Messages] row {} col 2 (tool_input) error: {:?}", i, e);
            e
        })?;
        let tool_output = read_optional_text(row, 3).map_err(|e| {
            tracing::error!("[Messages] row {} col 3 (tool_output) error: {:?}", i, e);
            e
        })?;
        let created_at: String = row.try_get(4).map_err(|e| {
            tracing::error!("[Messages] row {} col 4 (created_at) error: {:?}", i, e);
            e
        })?;
        let tool_calls = tool_input
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());
        result.push(PersistedChatMessage {
            role,
            content,
            tool_calls,
            tool_call_id: tool_output,
            created_at,
        });
    }
    Ok(result)
}

pub async fn delete(pool: &DbPool, session_id: &str, owner_id: &str) -> AppResult<bool> {
    // 不依赖数据库是否开启外键级联：MySQL 旧表结构没有声明级联约束，
    // 直接删除 sessions 会留下消息和产物孤儿记录。先校验所有权，再在
    // 同一事务内清理子表，保证删除结果与 UI 列表保持一致。
    let mut tx = pool.begin().await?;
    let session = sqlx::query("SELECT id FROM sessions WHERE id = ? AND owner_id = ?")
        .bind(session_id)
        .bind(owner_id)
        .fetch_optional(&mut *tx)
        .await?;

    if session.is_none() {
        tx.rollback().await?;
        return Ok(false);
    }

    sqlx::query("DELETE FROM messages WHERE session_id = ?")
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM artifact_versions WHERE session_id = ?")
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM session_artifacts WHERE session_id = ?")
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    let result = sqlx::query("DELETE FROM sessions WHERE id = ? AND owner_id = ?")
        .bind(session_id)
        .bind(owner_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(result.rows_affected() > 0)
}

pub async fn clear_messages(pool: &DbPool, session_id: &str, owner_id: &str) -> AppResult<bool> {
    // 验证所有权
    let session = sqlx::query("SELECT id FROM sessions WHERE id = ? AND owner_id = ?")
        .bind(session_id)
        .bind(owner_id)
        .fetch_optional(pool)
        .await?;

    if session.is_none() {
        return Ok(false);
    }

    sqlx::query("DELETE FROM messages WHERE session_id = ?")
        .bind(session_id)
        .execute(pool)
        .await?;

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE sessions SET message_count = 0, process_events = ?, updated_at = ? WHERE id = ?",
    )
    .bind("[]")
    .bind(&now)
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(true)
}
