use crate::db::DbPool;
use crate::error::AppResult;
use chrono::{DateTime, Utc};
use sqlx::Row;

#[derive(Debug)]
pub struct PasswordResetToken {
    pub user_id: String,
    pub expires_at: String,
    pub used_at: Option<String>,
}

pub async fn has_recent_request(pool: &DbPool, user_id: &str, since: &str) -> AppResult<bool> {
    let row = sqlx::query(
        "SELECT id FROM password_reset_tokens WHERE user_id = ? AND created_at >= ? LIMIT 1",
    )
    .bind(user_id)
    .bind(since)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

pub async fn create(
    pool: &DbPool,
    user_id: &str,
    token_hash: &str,
    expires_at: &str,
    now: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE password_reset_tokens SET used_at = ? WHERE user_id = ? AND used_at IS NULL",
    )
    .bind(now)
    .bind(user_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO password_reset_tokens (id, user_id, token_hash, expires_at, used_at, created_at) VALUES (?, ?, ?, ?, NULL, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn consume_and_update_password(
    pool: &DbPool,
    token_hash: &str,
    password_hash: &str,
    now: DateTime<Utc>,
) -> AppResult<bool> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT user_id, expires_at, used_at FROM password_reset_tokens WHERE token_hash = ?",
    )
    .bind(token_hash)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.rollback().await?;
        return Ok(false);
    };

    let user_id: String = row.try_get(0)?;
    let expires_at: String = row.try_get(1)?;
    let used_at: Option<String> = row.try_get(2)?;
    let expires_at = DateTime::parse_from_rfc3339(&expires_at)
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now() - chrono::Duration::seconds(1));

    if used_at.is_some() || expires_at <= now {
        tx.rollback().await?;
        return Ok(false);
    }

    let consumed = sqlx::query(
        "UPDATE password_reset_tokens SET used_at = ? WHERE token_hash = ? AND used_at IS NULL",
    )
    .bind(now.to_rfc3339())
    .bind(token_hash)
    .execute(&mut *tx)
    .await?;

    if consumed.rows_affected() != 1 {
        tx.rollback().await?;
        return Ok(false);
    }

    let updated_at = now.to_rfc3339();
    let updated = crate::db::user_repo::update_password_and_bump_auth_version(
        &mut *tx,
        &user_id,
        password_hash,
        &updated_at,
    )
    .await?;

    if updated != 1 {
        tx.rollback().await?;
        return Ok(false);
    }

    tx.commit().await?;
    Ok(true)
}
