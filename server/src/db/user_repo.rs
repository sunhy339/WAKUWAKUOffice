use super::DbPool;
use crate::error::AppResult;
use crate::models::User;

pub async fn find_by_username(pool: &DbPool, username: &str) -> AppResult<Option<(User, String)>> {
    let row = sqlx::query(
        "SELECT id, username, email, password_hash, avatar, role, auth_version FROM users WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let id: String = r.try_get(0)?;
            let username: String = r.try_get(1)?;
            let email: Option<String> = r.try_get(2)?;
            let password_hash: String = r.try_get(3)?;
            let avatar: Option<String> = r.try_get(4)?;
            let role: String = r.try_get(5)?;
            let auth_version: i64 = r.try_get(6)?;
            Ok(Some((
                User {
                    id,
                    username,
                    email,
                    avatar,
                    role,
                    auth_version,
                },
                password_hash,
            )))
        }
        None => Ok(None),
    }
}

pub async fn find_by_id(pool: &DbPool, id: &str) -> AppResult<Option<User>> {
    let row = sqlx::query(
        "SELECT id, username, email, avatar, role, auth_version FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let user = User {
                id: r.try_get(0)?,
                username: r.try_get(1)?,
                email: r.try_get(2)?,
                avatar: r.try_get(3)?,
                role: r.try_get(4)?,
                auth_version: r.try_get(5)?,
            };
            Ok(Some(user))
        }
        None => Ok(None),
    }
}

pub async fn create(
    pool: &DbPool,
    username: &str,
    email: Option<&str>,
    password_hash: &str,
) -> AppResult<User> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, role, auth_version, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'user', 0, ?, ?)"
    )
    .bind(&id)
    .bind(username)
    .bind(email)
    .bind(password_hash)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(User {
        id,
        username: username.to_string(),
        email: email.map(|s| s.to_string()),
        avatar: None,
        role: "user".to_string(),
        auth_version: 0,
    })
}

pub async fn find_by_email(pool: &DbPool, email: &str) -> AppResult<Option<User>> {
    let row = sqlx::query(
        "SELECT id, username, email, avatar, role, auth_version FROM users WHERE email = ?",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(Some(User {
            id: r.try_get(0)?,
            username: r.try_get(1)?,
            email: r.try_get(2)?,
            avatar: r.try_get(3)?,
            role: r.try_get(4)?,
            auth_version: r.try_get(5)?,
        })),
        None => Ok(None),
    }
}

pub async fn update_password_and_bump_auth_version(
    executor: impl sqlx::Executor<'_, Database = sqlx::Any>,
    user_id: &str,
    password_hash: &str,
    updated_at: &str,
) -> AppResult<u64> {
    let result = sqlx::query(
        "UPDATE users SET password_hash = ?, auth_version = auth_version + 1, updated_at = ? WHERE id = ?",
    )
    .bind(password_hash)
    .bind(updated_at)
    .bind(user_id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

pub async fn find_or_create_external(pool: &DbPool, username: &str) -> AppResult<User> {
    if let Some((user, _)) = find_by_username(pool, username).await? {
        return Ok(user);
    }

    let password_hash = hash_password(&uuid::Uuid::new_v4().to_string())?;
    create(pool, username, None, &password_hash).await
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    bcrypt::verify(password, hash).unwrap_or(false)
}

pub fn hash_password(password: &str) -> AppResult<String> {
    Ok(bcrypt::hash(password, 10)?)
}

use sqlx::Row;
