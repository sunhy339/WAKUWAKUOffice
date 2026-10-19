use axum::routing::{get, post};
use axum::{Json, Router};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::middleware::AuthUser;
use crate::auth::password_reset;
use crate::db::user_repo;
use crate::error::AppError;
use crate::models::{
    AuthModeResponse, LoginRequest, PasswordResetConfirmRequest, PasswordResetRequest,
    PasswordResetResponse, RegisterRequest, TokenResponse, VerificationLoginRequest,
};
use crate::state;

pub fn router() -> Router {
    Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/mode", get(auth_mode))
        .route("/api/auth/verification-login", post(verification_login))
        .route("/api/auth/register", post(register))
        .route("/api/auth/local-session", post(local_session))
        .route(
            "/api/auth/password-reset/request",
            post(request_password_reset),
        )
        .route(
            "/api/auth/password-reset/confirm",
            post(confirm_password_reset),
        )
        .route("/api/auth/me", get(me))
}

async fn auth_mode() -> Json<AuthModeResponse> {
    Json(AuthModeResponse {
        mode: crate::config::config().auth_mode.clone(),
    })
}

async fn login(Json(req): Json<LoginRequest>) -> Result<Json<TokenResponse>, AppError> {
    let pool = state::db_pool();
    let (user, hash) = user_repo::find_by_username(&pool, &req.username)
        .await?
        .ok_or(AppError::BadRequest("用户名或密码错误".into()))?;

    if !user_repo::verify_password(&hash, &req.password) {
        return Err(AppError::BadRequest("用户名或密码错误".into()));
    }

    let token = crate::auth::create_token(&user)?;
    Ok(Json(TokenResponse {
        access_token: token,
        token_type: "bearer".into(),
        user,
    }))
}

#[derive(Debug, Deserialize)]
struct XApiLoginResponse {
    code: String,
    #[serde(default)]
    info: String,
    #[serde(default)]
    data: Option<String>,
}

async fn verification_login(
    Json(req): Json<VerificationLoginRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    login_with_verification(req).await
}

async fn login_with_verification(
    req: VerificationLoginRequest,
) -> Result<Json<TokenResponse>, AppError> {
    let code = req.code.trim();
    if code.is_empty() {
        return Err(AppError::BadRequest("请输入验证码".into()));
    }

    let cfg = crate::config::config();
    let resp = reqwest::Client::new()
        .post(&cfg.x_api_auth_login_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[("code", code)])
        .send()
        .await
        .map_err(|_| AppError::BadRequest("验证码服务暂不可用".into()))?;

    let result = resp
        .json::<XApiLoginResponse>()
        .await
        .map_err(|_| AppError::BadRequest("验证码服务响应异常".into()))?;

    if result.code != "0000" {
        return Err(AppError::BadRequest(if result.info.is_empty() {
            "验证码无效或已过期".into()
        } else {
            result.info
        }));
    }

    let pool = state::db_pool();
    let external_id = result
        .data
        .as_deref()
        .and_then(extract_openid_from_x_api_token)
        .unwrap_or_else(|| code.to_string());
    let username = format!("wx_{external_id}");
    let user = user_repo::find_or_create_external(&pool, &username).await?;
    let token = crate::auth::create_token(&user)?;

    Ok(Json(TokenResponse {
        access_token: token,
        token_type: "bearer".into(),
        user,
    }))
}

fn extract_openid_from_x_api_token(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("openId")?.as_str().map(ToString::to_string)
}

async fn register(Json(req): Json<RegisterRequest>) -> Result<Json<TokenResponse>, AppError> {
    validate_username(&req.username)?;
    validate_password(&req.password)?;

    let username = req.username.trim();
    let email = normalize_email(req.email.as_deref())?;

    let pool = state::db_pool();
    if user_repo::find_by_username(&pool, username)
        .await?
        .is_some()
    {
        return Err(AppError::BadRequest("用户名已存在".into()));
    }
    if let Some(email) = email.as_deref() {
        if user_repo::find_by_email(&pool, email).await?.is_some() {
            return Err(AppError::BadRequest("邮箱已被注册".into()));
        }
    }

    let hash = user_repo::hash_password(&req.password)?;
    let user = user_repo::create(&pool, username, email.as_deref(), &hash).await?;
    let token = crate::auth::create_token(&user)?;

    Ok(Json(TokenResponse {
        access_token: token,
        token_type: "bearer".into(),
        user,
    }))
}

async fn local_session() -> Result<Json<TokenResponse>, AppError> {
    let cfg = crate::config::config();
    if !cfg.is_local_auth() {
        return Err(AppError::Forbidden);
    }

    let pool = state::db_pool();
    let user = if let Some((user, _)) =
        user_repo::find_by_username(&pool, &cfg.auth_local_username).await?
    {
        user
    } else {
        let hash = user_repo::hash_password(&uuid::Uuid::new_v4().to_string())?;
        user_repo::create(&pool, &cfg.auth_local_username, None, &hash).await?
    };
    let token = crate::auth::create_token(&user)?;

    Ok(Json(TokenResponse {
        access_token: token,
        token_type: "bearer".into(),
        user,
    }))
}

#[derive(Debug, Serialize)]
struct PasswordResetWebhookPayload {
    to: String,
    subject: String,
    reset_url: String,
    expires_in_minutes: i64,
}

async fn request_password_reset(
    Json(req): Json<PasswordResetRequest>,
) -> Result<Json<PasswordResetResponse>, AppError> {
    let generic_message = "如果该邮箱已注册，重置链接会发送到邮箱".to_string();
    let email = normalize_email(Some(&req.email))?;
    let Some(email) = email else {
        return Ok(Json(PasswordResetResponse {
            message: generic_message,
            reset_url: None,
        }));
    };

    let cfg = crate::config::config();
    let pool = state::db_pool();
    let Some(user) = user_repo::find_by_email(&pool, &email).await? else {
        return Ok(Json(PasswordResetResponse {
            message: generic_message,
            reset_url: None,
        }));
    };

    let now = Utc::now();
    let recent_since = (now - Duration::seconds(60)).to_rfc3339();
    if crate::db::password_reset_repo::has_recent_request(&pool, &user.id, &recent_since).await? {
        return Ok(Json(PasswordResetResponse {
            message: generic_message,
            reset_url: None,
        }));
    }

    let token = password_reset::generate_token();
    let reset_url = password_reset::build_reset_url(&cfg.auth_reset_base_url, &token);
    let expires_at = (now + Duration::minutes(cfg.auth_reset_token_ttl_minutes)).to_rfc3339();
    crate::db::password_reset_repo::create(
        &pool,
        &user.id,
        &password_reset::hash_token(&token),
        &expires_at,
        &now.to_rfc3339(),
    )
    .await?;

    if let Some(webhook_url) = cfg.auth_reset_webhook_url.as_deref() {
        let payload = PasswordResetWebhookPayload {
            to: email,
            subject: "WaKuOffice 密码重置".into(),
            reset_url: reset_url.clone(),
            expires_in_minutes: cfg.auth_reset_token_ttl_minutes,
        };
        if let Err(error) = reqwest::Client::new()
            .post(webhook_url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            tracing::warn!("密码重置通知发送失败: {error}");
        }
    }

    Ok(Json(PasswordResetResponse {
        message: generic_message,
        reset_url: cfg.auth_reset_dev_mode.then_some(reset_url),
    }))
}

async fn confirm_password_reset(
    Json(req): Json<PasswordResetConfirmRequest>,
) -> Result<Json<PasswordResetResponse>, AppError> {
    if req.token.trim().len() < 32 {
        return Err(AppError::BadRequest("重置链接无效或已过期".into()));
    }
    validate_password(&req.password)?;

    let password_hash = user_repo::hash_password(&req.password)?;
    let consumed = crate::db::password_reset_repo::consume_and_update_password(
        &state::db_pool(),
        &password_reset::hash_token(req.token.trim()),
        &password_hash,
        Utc::now(),
    )
    .await?;

    if !consumed {
        return Err(AppError::BadRequest("重置链接无效或已过期".into()));
    }

    Ok(Json(PasswordResetResponse {
        message: "密码已重置，请使用新密码登录".into(),
        reset_url: None,
    }))
}

fn validate_username(username: &str) -> Result<(), AppError> {
    let value = username.trim();
    if value.len() < 3 {
        return Err(AppError::BadRequest("用户名至少 3 个字符".into()));
    }
    if value.len() > 64 || value.chars().any(char::is_whitespace) {
        return Err(AppError::BadRequest("用户名长度或格式不正确".into()));
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 6 {
        return Err(AppError::BadRequest("密码至少 6 个字符".into()));
    }
    if password.len() > 72 {
        return Err(AppError::BadRequest("密码不能超过 72 个字符".into()));
    }
    Ok(())
}

fn normalize_email(email: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(value) = email.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() > 254 || !value.contains('@') || value.starts_with('@') || value.ends_with('@') {
        return Err(AppError::BadRequest("邮箱格式不正确".into()));
    }
    Ok(Some(value.to_lowercase()))
}

async fn me(user: AuthUser) -> Result<Json<crate::models::User>, AppError> {
    Ok(Json(user.0))
}
