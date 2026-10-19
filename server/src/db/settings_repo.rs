use crate::error::AppResult;
use crate::models::AppSettings;
use sqlx::Row;

use super::DbPool;

pub async fn find_by_user(pool: &DbPool, user_id: &str) -> AppResult<Option<AppSettings>> {
    let row = sqlx::query("SELECT payload FROM user_settings WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

    match row {
        Some(r) => {
            // SQLite migration 将 payload 定义为 TEXT；按 Vec<u8> 解码会在
            // 本地数据库上触发类型不匹配并让 /api/settings 返回 500。
            // 直接读取 String，同时兼容驱动返回的文本值。
            let payload: String = r.try_get(0)?;
            Ok(Some(serde_json::from_str::<AppSettings>(&payload)?))
        }
        None => Ok(None),
    }
}

pub async fn save_for_user(
    pool: &DbPool,
    user_id: &str,
    settings: &AppSettings,
) -> AppResult<AppSettings> {
    let now = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::to_string(settings)?;

    let result =
        sqlx::query("UPDATE user_settings SET payload = ?, updated_at = ? WHERE user_id = ?")
            .bind(&payload)
            .bind(&now)
            .bind(user_id)
            .execute(pool)
            .await?;

    if result.rows_affected() == 0 {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO user_settings (id, user_id, payload, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(user_id)
        .bind(&payload)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    Ok(settings.clone())
}
