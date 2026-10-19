use axum::routing::{get, post};
use axum::{Json, Router};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::middleware::AuthUser;
use crate::db::settings_repo;
use crate::error::AppError;
use crate::llm::client::LlmClient;
use crate::models::ChatMessage;
use crate::models::{AppSettings, BasicSettings, LlmProfileConfig, McpServerConfig};
use crate::state;
use eventsource_stream::Eventsource;
use futures::StreamExt;

pub fn router() -> Router {
    Router::new()
        .route("/api/settings", get(get_settings).put(save_settings))
        .route("/api/settings/models", post(fetch_models))
        .route("/api/settings/llm/test", post(test_llm_connection))
        .route("/api/settings/mcp/test", post(test_mcp_service))
}

#[derive(Debug, Deserialize)]
struct FetchModelsRequest {
    base_url: String,
    #[serde(default)]
    api_keys: Vec<String>,
    #[serde(default)]
    api_key: Option<String>,
}

#[derive(Debug, Serialize)]
struct FetchModelsResponse {
    models: Vec<String>,
    endpoint: String,
}

#[derive(Debug, Deserialize)]
struct TestLlmRequest {
    base_url: String,
    #[serde(default)]
    api_keys: Vec<String>,
    #[serde(default)]
    api_key: Option<String>,
    model: String,
    #[serde(default)]
    wire_api: String,
    #[serde(default)]
    reasoning_effort: String,
}

#[derive(Debug, Serialize)]
struct TestLlmResponse {
    ok: bool,
    message: String,
    model: String,
    wire_api: String,
    elapsed_ms: u128,
}

fn models_endpoint(base_url: &str) -> Result<String, AppError> {
    let mut endpoint = base_url.trim().trim_end_matches('/').to_string();
    if endpoint.is_empty() {
        return Err(AppError::BadRequest("Base URL 不能为空".into()));
    }
    if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
        return Err(AppError::BadRequest(
            "Base URL 必须以 http:// 或 https:// 开头".into(),
        ));
    }
    if endpoint.ends_with("/chat/completions") {
        endpoint.truncate(endpoint.len() - "/chat/completions".len());
    }
    if !endpoint.ends_with("/models") {
        endpoint.push_str("/models");
    }
    reqwest::Url::parse(&endpoint)
        .map_err(|_| AppError::BadRequest("Base URL 格式不正确".into()))?;
    Ok(endpoint)
}

fn unique_api_keys(payload: &FetchModelsRequest) -> Vec<String> {
    let mut keys = payload.api_keys.clone();
    if let Some(api_key) = &payload.api_key {
        keys.push(api_key.clone());
    }
    let mut unique = Vec::new();
    for key in keys {
        let key = key.trim();
        if !key.is_empty() && !unique.iter().any(|item: &String| item == key) {
            unique.push(key.to_string());
        }
    }
    unique
}

fn model_ids(value: serde_json::Value) -> Vec<String> {
    let candidates = if let Some(items) = value.as_array() {
        items.clone()
    } else {
        value
            .get("data")
            .and_then(|items| items.as_array())
            .or_else(|| value.get("models").and_then(|items| items.as_array()))
            .cloned()
            .unwrap_or_default()
    };

    let mut models = Vec::new();
    for item in candidates {
        let model = item
            .as_str()
            .map(str::to_string)
            .or_else(|| {
                item.get("id")
                    .and_then(|id| id.as_str())
                    .map(str::to_string)
            })
            .or_else(|| {
                item.get("name")
                    .and_then(|name| name.as_str())
                    .map(str::to_string)
            });
        if let Some(model) = model {
            let model = model.trim();
            if !model.is_empty() && !models.iter().any(|item| item == model) {
                models.push(model.to_string());
            }
        }
    }
    models.sort_by_key(|item| item.to_lowercase());
    models
}

async fn fetch_models(
    _user: AuthUser,
    Json(payload): Json<FetchModelsRequest>,
) -> Result<Json<FetchModelsResponse>, AppError> {
    let endpoint = models_endpoint(&payload.base_url)?;
    let keys = unique_api_keys(&payload);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| AppError::Internal(anyhow::anyhow!("构建模型查询客户端失败: {error}")))?;

    let attempts = if keys.is_empty() {
        vec![String::new()]
    } else {
        keys
    };
    let mut last_message = String::from("模型服务未返回可用结果");

    for (index, api_key) in attempts.iter().enumerate() {
        let mut request = client.get(&endpoint).header("Accept", "application/json");
        if !api_key.is_empty() {
            request = request.bearer_auth(api_key);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                last_message = format!("连接模型服务失败: {error}");
                if index + 1 < attempts.len() {
                    continue;
                }
                break;
            }
        };

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            last_message = format!("模型服务返回 HTTP {}", status.as_u16());
            if matches!(
                status,
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
            ) || status.is_server_error()
            {
                if index + 1 < attempts.len() {
                    continue;
                }
            }
            return Err(AppError::BadRequest(last_message));
        }

        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|_| AppError::BadRequest("模型服务返回的 /models 数据不是有效 JSON".into()))?;
        let models = model_ids(value);
        if models.is_empty() {
            return Err(AppError::BadRequest(
                "模型服务返回成功，但没有找到模型 ID".into(),
            ));
        }
        return Ok(Json(FetchModelsResponse { models, endpoint }));
    }

    Err(AppError::BadRequest(last_message))
}

async fn test_llm_connection(
    user: AuthUser,
    Json(payload): Json<TestLlmRequest>,
) -> Result<Json<TestLlmResponse>, AppError> {
    let base_url = payload.base_url.trim();
    let model = payload.model.trim();
    let wire_api = if payload.wire_api.trim().is_empty() {
        "chat_completions"
    } else {
        payload.wire_api.trim()
    };
    if base_url.is_empty() {
        return Err(AppError::BadRequest("Base URL 不能为空".into()));
    }
    if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
        return Err(AppError::BadRequest(
            "Base URL 必须以 http:// 或 https:// 开头".into(),
        ));
    }
    if model.is_empty() {
        return Err(AppError::BadRequest("请选择或填写要测试的模型".into()));
    }
    if !matches!(wire_api, "chat_completions" | "responses") {
        return Err(AppError::BadRequest(
            "Wire API 只支持 chat_completions 或 responses".into(),
        ));
    }

    let profile = LlmProfileConfig {
        id: "connection-test".into(),
        name: "连接测试".into(),
        base_url: base_url.to_string(),
        provider: "OpenAI Compatible".into(),
        wire_api: wire_api.to_string(),
        reasoning_effort: if payload.reasoning_effort.trim().is_empty() {
            "medium".into()
        } else {
            payload.reasoning_effort.trim().to_string()
        },
        api_keys: payload.api_keys,
        models: vec![model.to_string()],
        default_model: model.to_string(),
        api_key: payload.api_key,
        has_api_key: false,
    };

    let started = std::time::Instant::now();
    let client = LlmClient::from_profile(
        &profile,
        Some(model),
        &format!("user:{}:connection-test", user.0.id),
    );
    let messages = [ChatMessage {
        role: "user".into(),
        content: "请只回复：连接测试成功".into(),
        tool_calls: None,
        tool_call_id: None,
    }];

    match client.chat(&messages, None).await {
        Ok(response) if response.choices.first().is_some() => Ok(Json(TestLlmResponse {
            ok: true,
            message: "连接成功，模型已返回响应".into(),
            model: client.model().to_string(),
            wire_api: wire_api.to_string(),
            elapsed_ms: started.elapsed().as_millis(),
        })),
        Ok(_) => Ok(Json(TestLlmResponse {
            ok: false,
            message: "模型服务返回成功，但没有可用响应内容".into(),
            model: client.model().to_string(),
            wire_api: wire_api.to_string(),
            elapsed_ms: started.elapsed().as_millis(),
        })),
        Err(error) => Ok(Json(TestLlmResponse {
            ok: false,
            message: format!("连接失败：{error}"),
            model: client.model().to_string(),
            wire_api: wire_api.to_string(),
            elapsed_ms: started.elapsed().as_millis(),
        })),
    }
}

pub fn default_settings() -> AppSettings {
    let cfg = crate::config::config();
    let models = configured_models(cfg);
    let default_profile = LlmProfileConfig {
        id: "default".into(),
        name: "默认模型服务".into(),
        base_url: cfg.llm_base_url.clone(),
        provider: cfg.llm_text_provider.clone(),
        wire_api: cfg.llm_text_wire_api.clone(),
        reasoning_effort: cfg.llm_text_reasoning_effort.clone(),
        api_keys: if cfg.llm_api_key.trim().is_empty() {
            vec![]
        } else {
            vec![cfg.llm_api_key.clone()]
        },
        models,
        default_model: cfg.llm_model.clone(),
        api_key: None,
        has_api_key: !cfg.llm_api_key.is_empty(),
    };

    AppSettings {
        llm_profiles: vec![default_profile.clone()],
        active_profile_id: default_profile.id.clone(),
        default_model: default_profile.default_model.clone(),
        active_model: default_profile.default_model.clone(),
        basic: BasicSettings {
            app_name: cfg.app_name.clone(),
            workspace_title: "智能办公助手".into(),
            brand_tagline: "打开即用，专注办公创作".into(),
            default_theme: "default".into(),
        },
        mcp_servers: builtin_mcp_servers(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn configured_models(cfg: &crate::config::Config) -> Vec<String> {
    let mut models = Vec::new();
    for model in cfg
        .llm_text_models
        .iter()
        .chain(cfg.llm_image_models.iter())
        .chain(cfg.llm_video_models.iter())
    {
        let model = model.trim();
        if !model.is_empty() && !models.iter().any(|item| item == model) {
            models.push(model.to_string());
        }
    }
    models
}

fn builtin_mcp_servers() -> Vec<McpServerConfig> {
    vec![]
}

pub fn normalize_settings(mut settings: AppSettings) -> Result<AppSettings, AppError> {
    if settings.llm_profiles.is_empty() {
        return Err(AppError::BadRequest("至少保留一个模型服务配置".into()));
    }

    for profile in &mut settings.llm_profiles {
        if profile.id.trim().is_empty() {
            profile.id = uuid::Uuid::new_v4().to_string();
        }
        if profile.name.trim().is_empty() {
            profile.name = "未命名模型服务".into();
        }
        if profile.provider.trim().is_empty() {
            profile.provider = "OpenAI Compatible".into();
        }
        if profile.wire_api.trim().is_empty() {
            profile.wire_api = "chat_completions".into();
        }
        if !matches!(profile.wire_api.as_str(), "chat_completions" | "responses") {
            return Err(AppError::BadRequest(format!(
                "模型服务「{}」的 Wire API 只支持 chat_completions 或 responses",
                profile.name
            )));
        }
        if profile.reasoning_effort.trim().is_empty() {
            profile.reasoning_effort = "medium".into();
        }
        let mut api_keys: Vec<String> = Vec::new();
        for api_key in &profile.api_keys {
            let api_key = api_key.trim().to_string();
            if !api_key.is_empty() && !api_keys.iter().any(|item| item == &api_key) {
                api_keys.push(api_key);
            }
        }
        if let Some(api_key) = profile.api_key.take() {
            let api_key = api_key.trim().to_string();
            if !api_key.is_empty() && !api_keys.iter().any(|item| item == &api_key) {
                api_keys.push(api_key);
            }
        }
        profile.api_keys = api_keys;
        profile.models = profile
            .models
            .iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect();
        if profile.models.is_empty() {
            return Err(AppError::BadRequest(format!(
                "模型服务「{}」至少需要一个模型",
                profile.name
            )));
        }
        if profile.default_model.trim().is_empty()
            || !profile
                .models
                .iter()
                .any(|item| item == &profile.default_model)
        {
            profile.default_model = profile.models[0].clone();
        }
        profile.has_api_key = !profile.api_keys.is_empty();
    }

    if !settings
        .llm_profiles
        .iter()
        .any(|profile| profile.id == settings.active_profile_id)
    {
        settings.active_profile_id = settings.llm_profiles[0].id.clone();
    }

    let active_profile = settings
        .llm_profiles
        .iter()
        .find(|profile| profile.id == settings.active_profile_id)
        .cloned()
        .unwrap_or_else(|| settings.llm_profiles[0].clone());

    if settings.default_model.trim().is_empty()
        || !active_profile
            .models
            .iter()
            .any(|item| item == &settings.default_model)
    {
        settings.default_model = active_profile.default_model.clone();
    }
    if settings.active_model.trim().is_empty()
        || !active_profile
            .models
            .iter()
            .any(|item| item == &settings.active_model)
    {
        settings.active_model = settings.default_model.clone();
    }

    if settings.basic.app_name.trim().is_empty() {
        settings.basic.app_name = crate::config::config().app_name.clone();
    }
    if settings.basic.app_name.eq_ignore_ascii_case("walioffice") {
        settings.basic.app_name = "WaKuOffice".into();
    }
    if settings.basic.workspace_title.trim().is_empty() {
        settings.basic.workspace_title = "智能办公助手".into();
    }
    if settings.basic.brand_tagline.trim().is_empty() {
        settings.basic.brand_tagline = "打开即用，专注办公创作".into();
    }
    if settings.basic.default_theme.trim().is_empty() {
        settings.basic.default_theme = "default".into();
    }

    for builtin in builtin_mcp_servers() {
        if !settings
            .mcp_servers
            .iter()
            .any(|server| server.id == builtin.id || server.endpoint == builtin.endpoint)
        {
            settings.mcp_servers.push(builtin);
        }
    }

    settings.updated_at = chrono::Utc::now().to_rfc3339();
    Ok(settings)
}

pub fn import_startup_llm_profile(settings: &mut AppSettings) {
    let cfg = crate::config::config();
    let base_url = cfg.llm_base_url.trim();
    let model = cfg.llm_model.trim();
    if base_url.is_empty() || model.is_empty() {
        return;
    }

    let startup_keys = if cfg.llm_api_key.trim().is_empty() {
        vec![]
    } else {
        vec![cfg.llm_api_key.trim().to_string()]
    };

    if let Some(profile) = settings.llm_profiles.iter_mut().find(|profile| {
        profile.id == "startup-env"
            || profile.base_url.trim_end_matches('/') == base_url.trim_end_matches('/')
    }) {
        if profile.id == "startup-env" {
            profile.name = "启动配置模型服务".into();
            profile.base_url = base_url.to_string();
        }
        if !profile.models.iter().any(|item| item == model) {
            profile.models.insert(0, model.to_string());
        }
        for configured in configured_models(cfg) {
            if !profile.models.iter().any(|item| item == &configured) {
                profile.models.push(configured);
            }
        }
        for api_key in startup_keys {
            if !profile.api_keys.iter().any(|item| item == &api_key) {
                profile.api_keys.push(api_key);
            }
        }
        if profile.default_model.trim().is_empty() {
            profile.default_model = model.to_string();
        }
        return;
    }

    let models = configured_models(cfg);

    settings.llm_profiles.push(LlmProfileConfig {
        id: "startup-env".into(),
        name: "启动配置模型服务".into(),
        base_url: base_url.to_string(),
        provider: cfg.llm_text_provider.clone(),
        wire_api: cfg.llm_text_wire_api.clone(),
        reasoning_effort: cfg.llm_text_reasoning_effort.clone(),
        api_keys: startup_keys,
        models,
        default_model: model.to_string(),
        api_key: None,
        has_api_key: !cfg.llm_api_key.trim().is_empty(),
    });
}

async fn get_settings(user: AuthUser) -> Result<Json<AppSettings>, AppError> {
    let pool = state::db_pool();
    let settings = settings_repo::find_by_user(&pool, &user.0.id)
        .await?
        .unwrap_or_else(default_settings);
    Ok(Json(normalize_settings(settings)?))
}

async fn save_settings(
    user: AuthUser,
    Json(payload): Json<AppSettings>,
) -> Result<Json<AppSettings>, AppError> {
    let pool = state::db_pool();
    let normalized = normalize_settings(payload)?;
    let saved = settings_repo::save_for_user(&pool, &user.0.id, &normalized).await?;
    Ok(Json(saved))
}

async fn test_mcp_service(
    _user: AuthUser,
    Json(payload): Json<McpServerConfig>,
) -> Result<Json<serde_json::Value>, AppError> {
    if payload.transport == "sse" {
        return test_sse_mcp_service(&payload).await;
    }

    if payload.transport != "http" {
        return Ok(Json(json!({
            "ok": false,
            "message": format!("当前仅支持测试 HTTP 类型 MCP 服务，暂不支持 {}", payload.transport),
            "tools": []
        })));
    }

    let endpoint = payload.endpoint.trim().trim_end_matches('/').to_string();
    if endpoint.is_empty() {
        return Err(AppError::BadRequest("MCP 服务地址不能为空".into()));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("构建 MCP 测试客户端失败: {e}")))?;

    let initialize_resp = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "WaKuOffice",
                    "version": "0.2.0"
                }
            }
        }))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("连接 MCP 服务失败: {e}")))?;

    let status = initialize_resp.status();
    let init_text = initialize_resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Ok(Json(json!({
            "ok": false,
            "message": format!("初始化失败: HTTP {} {}", status.as_u16(), init_text),
            "tools": []
        })));
    }

    let _ = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }))
        .send()
        .await;

    let tools_resp = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("读取 MCP 工具列表失败: {e}")))?;

    let tools_status = tools_resp.status();
    let tools_text = tools_resp.text().await.unwrap_or_default();
    if !tools_status.is_success() {
        return Ok(Json(json!({
            "ok": false,
            "message": format!("获取工具列表失败: HTTP {} {}", tools_status.as_u16(), tools_text),
            "tools": []
        })));
    }

    let payload_value: serde_json::Value = serde_json::from_str(&tools_text)
        .map_err(|e| AppError::BadRequest(format!("解析 MCP 工具列表失败: {e}")))?;
    let tools = payload_value
        .get("result")
        .and_then(|result| result.get("tools"))
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(Json(json!({
        "ok": true,
        "message": format!("MCP 服务连接成功，共发现 {} 个工具", tools.len()),
        "tools": tools
    })))
}

async fn test_sse_mcp_service(
    payload: &McpServerConfig,
) -> Result<Json<serde_json::Value>, AppError> {
    let endpoint = payload.endpoint.trim().to_string();
    if endpoint.is_empty() {
        return Err(AppError::BadRequest("MCP SSE 地址不能为空".into()));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("构建 MCP SSE 客户端失败: {e}")))?;

    let sse_resp = client
        .get(&endpoint)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("连接 MCP SSE 服务失败: {e}")))?;

    if !sse_resp.status().is_success() {
        let status = sse_resp.status();
        let body = sse_resp.text().await.unwrap_or_default();
        return Ok(Json(json!({
            "ok": false,
            "message": format!("连接 MCP SSE 服务失败: HTTP {} {}", status.as_u16(), body),
            "tools": []
        })));
    }

    let mut stream = sse_resp.bytes_stream().eventsource();
    let message_endpoint = loop {
        let Some(event) = stream.next().await else {
            return Ok(Json(json!({
                "ok": false,
                "message": "MCP SSE 服务未返回 endpoint 事件",
                "tools": []
            })));
        };

        match event {
            Ok(ev) if ev.event == "endpoint" && !ev.data.trim().is_empty() => {
                break if ev.data.starts_with("http://") || ev.data.starts_with("https://") {
                    ev.data
                } else {
                    return Ok(Json(json!({
                        "ok": false,
                        "message": "MCP SSE 服务返回了非绝对 endpoint",
                        "tools": []
                    })));
                };
            }
            Ok(_) => {}
            Err(err) => {
                return Ok(Json(json!({
                    "ok": false,
                    "message": format!("解析 MCP SSE 事件失败: {err}"),
                    "tools": []
                })));
            }
        }
    };

    let request_headers = [
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
    ];

    let initialize_resp = client
        .post(&message_endpoint)
        .headers(request_headers.iter().fold(
            reqwest::header::HeaderMap::new(),
            |mut headers, (key, value)| {
                headers.insert(*key, reqwest::header::HeaderValue::from_static(value));
                headers
            },
        ))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "WaKuOffice",
                    "version": "0.2.0"
                }
            }
        }))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("发送 MCP initialize 失败: {e}")))?;

    if !initialize_resp.status().is_success() {
        return Ok(Json(json!({
            "ok": false,
            "message": format!("MCP initialize 失败: HTTP {}", initialize_resp.status().as_u16()),
            "tools": []
        })));
    }

    let _ = client
        .post(&message_endpoint)
        .header("Content-Type", "application/json")
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }))
        .send()
        .await;

    let tools_resp = client
        .post(&message_endpoint)
        .headers(request_headers.iter().fold(
            reqwest::header::HeaderMap::new(),
            |mut headers, (key, value)| {
                headers.insert(*key, reqwest::header::HeaderValue::from_static(value));
                headers
            },
        ))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("请求 MCP 工具列表失败: {e}")))?;

    if !tools_resp.status().is_success() {
        return Ok(Json(json!({
            "ok": false,
            "message": format!("MCP tools/list 失败: HTTP {}", tools_resp.status().as_u16()),
            "tools": []
        })));
    }

    let tools = loop {
        let Some(event) = stream.next().await else {
            return Ok(Json(json!({
                "ok": false,
                "message": "MCP SSE 未返回工具列表结果",
                "tools": []
            })));
        };

        match event {
            Ok(ev) if ev.event == "message" => {
                let payload_value: serde_json::Value = match serde_json::from_str(&ev.data) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if payload_value.get("id").and_then(|v| v.as_i64()) == Some(2) {
                    break payload_value
                        .get("result")
                        .and_then(|result| result.get("tools"))
                        .and_then(|value| value.as_array())
                        .cloned()
                        .unwrap_or_default();
                }
            }
            Ok(_) => {}
            Err(err) => {
                return Ok(Json(json!({
                    "ok": false,
                    "message": format!("读取 MCP SSE 工具列表结果失败: {err}"),
                    "tools": []
                })));
            }
        }
    };

    Ok(Json(json!({
        "ok": true,
        "message": format!("MCP SSE 服务连接成功，共发现 {} 个工具", tools.len()),
        "tools": tools
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_endpoint_normalizes_openai_compatible_urls() {
        assert_eq!(
            models_endpoint("https://api.example.com/v1").unwrap(),
            "https://api.example.com/v1/models"
        );
        assert_eq!(
            models_endpoint("https://api.example.com/v1/").unwrap(),
            "https://api.example.com/v1/models"
        );
        assert_eq!(
            models_endpoint("https://api.example.com/v1/chat/completions").unwrap(),
            "https://api.example.com/v1/models"
        );
        assert!(models_endpoint("api.example.com/v1").is_err());
    }

    #[test]
    fn model_ids_support_common_provider_shapes_and_dedupe() {
        assert_eq!(
            model_ids(serde_json::json!({
                "data": [{"id": "gpt-b"}, {"id": "gpt-a"}, {"id": "gpt-a"}]
            })),
            vec!["gpt-a", "gpt-b"]
        );
        assert_eq!(
            model_ids(serde_json::json!({"models": ["model-b", "model-a"]})),
            vec!["model-a", "model-b"]
        );
        assert_eq!(
            model_ids(serde_json::json!([{"name": "named-model"}, "plain-model"])),
            vec!["named-model", "plain-model"]
        );
    }

    #[test]
    fn unique_api_keys_trims_and_dedupes_legacy_key() {
        let payload = FetchModelsRequest {
            base_url: "https://api.example.com/v1".into(),
            api_keys: vec![" key-a ".into(), "key-a".into(), "".into()],
            api_key: Some("key-b".into()),
        };
        assert_eq!(unique_api_keys(&payload), vec!["key-a", "key-b"]);
    }
}
