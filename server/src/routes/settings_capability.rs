use axum::Json;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

use crate::auth::middleware::AuthUser;
use crate::error::AppError;
use crate::models::ModelCapability;

#[derive(Debug, Deserialize)]
pub struct TestModelCapabilitiesRequest {
    pub base_url: String,
    #[serde(default)]
    pub api_keys: Vec<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    pub model: String,
    #[serde(default)]
    pub wire_api: String,
    #[serde(default)]
    pub reasoning_effort: String,
    #[serde(default)]
    pub capabilities: Vec<ModelCapability>,
}

#[derive(Debug, Serialize)]
pub struct CapabilityTestResult {
    pub capability: ModelCapability,
    pub ok: bool,
    pub message: String,
    pub elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
pub struct TestModelCapabilitiesResponse {
    pub ok: bool,
    pub model: String,
    pub results: Vec<CapabilityTestResult>,
    pub elapsed_ms: u128,
}

const CAPABILITY_TEST_IMAGE_DATA_URL: &str =
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

pub async fn test_model_capabilities(
    _user: AuthUser,
    Json(payload): Json<TestModelCapabilitiesRequest>,
) -> Result<Json<TestModelCapabilitiesResponse>, AppError> {
    let base_url = normalize_capability_base_url(&payload.base_url)?;
    let model = payload.model.trim();
    if model.is_empty() {
        return Err(AppError::BadRequest("请选择或填写要测试的模型".into()));
    }

    let wire_api = if payload.wire_api.trim().is_empty() {
        "chat_completions"
    } else {
        payload.wire_api.trim()
    };
    if !matches!(wire_api, "chat_completions" | "responses") {
        return Err(AppError::BadRequest(
            "Wire API 只支持 chat_completions 或 responses".into(),
        ));
    }

    let mut capabilities = payload.capabilities;
    capabilities.sort_by_key(|capability| capability.as_str());
    capabilities.dedup();
    if capabilities.is_empty() {
        return Err(AppError::BadRequest("请至少勾选一项模型能力".into()));
    }
    if capabilities.len() > 6 {
        return Err(AppError::BadRequest("一次最多测试 6 项模型能力".into()));
    }

    let keys = unique_api_keys(&payload.api_keys, payload.api_key.as_deref());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|error| AppError::Internal(anyhow::anyhow!("构建能力测试客户端失败: {error}")))?;
    let started = std::time::Instant::now();
    let mut results = Vec::with_capacity(capabilities.len());

    for capability in capabilities {
        let capability_started = std::time::Instant::now();
        let result = match capability {
            ModelCapability::Text | ModelCapability::ToolCalling => {
                test_text_capability(
                    &client,
                    &base_url,
                    &keys,
                    model,
                    wire_api,
                    payload.reasoning_effort.trim(),
                    capability,
                )
                .await
            }
            ModelCapability::Vision => {
                test_vision_capability(
                    &client,
                    &base_url,
                    &keys,
                    model,
                    wire_api,
                    payload.reasoning_effort.trim(),
                )
                .await
            }
            ModelCapability::ImageGeneration
            | ModelCapability::ImageEdit
            | ModelCapability::VideoGeneration => {
                test_media_capability(&client, &base_url, &keys, model, capability).await
            }
        };
        let (ok, message) = match result {
            Ok(()) => (true, format!("{}能力可用", capability_label(capability))),
            Err(error) => (
                false,
                format!("{}能力不可用：{error}", capability_label(capability)),
            ),
        };
        results.push(CapabilityTestResult {
            capability,
            ok,
            message,
            elapsed_ms: capability_started.elapsed().as_millis(),
        });
    }

    Ok(Json(TestModelCapabilitiesResponse {
        ok: results.iter().all(|result| result.ok),
        model: model.to_string(),
        results,
        elapsed_ms: started.elapsed().as_millis(),
    }))
}

fn normalize_capability_base_url(base_url: &str) -> Result<String, AppError> {
    let mut base = base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err(AppError::BadRequest("Base URL 不能为空".into()));
    }
    if !base.starts_with("http://") && !base.starts_with("https://") {
        return Err(AppError::BadRequest(
            "Base URL 必须以 http:// 或 https:// 开头".into(),
        ));
    }
    for suffix in [
        "/chat/completions",
        "/responses",
        "/images/generations",
        "/images/edits",
        "/videos",
    ] {
        if base.ends_with(suffix) {
            base.truncate(base.len() - suffix.len());
            break;
        }
    }
    reqwest::Url::parse(&base).map_err(|_| AppError::BadRequest("Base URL 格式不正确".into()))?;
    Ok(base)
}

fn capability_endpoint(base_url: &str, path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}{path}")
    } else {
        format!("{base}/v1{path}")
    }
}

fn unique_api_keys(api_keys: &[String], fallback: Option<&str>) -> Vec<String> {
    let mut values = api_keys.to_vec();
    if let Some(fallback) = fallback {
        values.push(fallback.to_string());
    }
    let mut result = Vec::new();
    for value in values {
        let value = value.trim();
        if !value.is_empty() && !result.iter().any(|item: &String| item == value) {
            result.push(value.to_string());
        }
    }
    result
}

async fn post_capability_json(
    client: &reqwest::Client,
    url: &str,
    keys: &[String],
    body: Value,
) -> Result<Value, String> {
    let attempts = if keys.is_empty() {
        vec![String::new()]
    } else {
        keys.to_vec()
    };
    let mut last_error = "模型服务未返回结果".to_string();

    for (index, api_key) in attempts.iter().enumerate() {
        let mut request = client
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&body);
        if !api_key.is_empty() {
            request = request.bearer_auth(api_key);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(_) => {
                last_error = "网络请求失败".into();
                if index + 1 < attempts.len() {
                    continue;
                }
                break;
            }
        };
        let status = response.status();
        let response_body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            last_error = format!("上游返回 HTTP {}", status.as_u16());
            if (status == StatusCode::UNAUTHORIZED
                || status == StatusCode::FORBIDDEN
                || status == StatusCode::TOO_MANY_REQUESTS
                || status.is_server_error())
                && index + 1 < attempts.len()
            {
                continue;
            }
            break;
        }
        return serde_json::from_str(&response_body)
            .map_err(|_| "上游返回的 JSON 无法解析".to_string());
    }

    Err(last_error)
}

fn capability_label(capability: ModelCapability) -> &'static str {
    match capability {
        ModelCapability::Text => "文本",
        ModelCapability::ToolCalling => "工具调用",
        ModelCapability::Vision => "视觉",
        ModelCapability::ImageGeneration => "文生图",
        ModelCapability::ImageEdit => "图生图",
        ModelCapability::VideoGeneration => "视频生成",
    }
}

fn capability_prompt(capability: ModelCapability) -> &'static str {
    match capability {
        ModelCapability::Text => "请只回复 OK。",
        ModelCapability::ToolCalling => "请调用 capability_probe 工具。",
        ModelCapability::Vision => "请只回复你看到的像素颜色。",
        ModelCapability::ImageGeneration => "A simple solid blue square, no text.",
        ModelCapability::ImageEdit => {
            "Edit this image into a slightly brighter blue square, no text."
        }
        ModelCapability::VideoGeneration => "A simple one-second blue light animation, no text.",
    }
}

fn response_has_chat_content(value: &Value, wire_api: &str) -> bool {
    if wire_api == "responses" {
        return value
            .get("output")
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or_else(|| value.get("output_text").and_then(Value::as_str).is_some());
    }
    value
        .get("choices")
        .and_then(Value::as_array)
        .map(|items| !items.is_empty())
        .unwrap_or(false)
}

async fn test_text_capability(
    client: &reqwest::Client,
    base_url: &str,
    keys: &[String],
    model: &str,
    wire_api: &str,
    reasoning_effort: &str,
    capability: ModelCapability,
) -> Result<(), String> {
    let prompt = capability_prompt(capability);
    let body = if wire_api == "responses" {
        let mut value = json!({
            "model": model,
            "input": [{"role": "user", "content": [{"type": "input_text", "text": prompt}]}],
            "max_output_tokens": 32,
        });
        if !reasoning_effort.is_empty() && reasoning_effort != "none" {
            value["reasoning"] = json!({"effort": reasoning_effort});
        }
        if capability == ModelCapability::ToolCalling {
            value["tools"] = json!([{
                "type": "function",
                "name": "capability_probe",
                "description": "Capability probe function",
                "parameters": {"type": "object", "properties": {}}
            }]);
        }
        value
    } else {
        let mut value = json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 32,
            "temperature": 0,
        });
        if capability == ModelCapability::ToolCalling {
            value["tools"] = json!([{
                "type": "function",
                "function": {
                    "name": "capability_probe",
                    "description": "Capability probe function",
                    "parameters": {"type": "object", "properties": {}}
                }
            }]);
        }
        value
    };
    let response = post_capability_json(
        client,
        &capability_endpoint(
            base_url,
            if wire_api == "responses" {
                "/responses"
            } else {
                "/chat/completions"
            },
        ),
        keys,
        body,
    )
    .await?;
    if response_has_chat_content(&response, wire_api) {
        Ok(())
    } else {
        Err("上游响应没有可用内容".into())
    }
}

async fn test_vision_capability(
    client: &reqwest::Client,
    base_url: &str,
    keys: &[String],
    model: &str,
    wire_api: &str,
    reasoning_effort: &str,
) -> Result<(), String> {
    let body = if wire_api == "responses" {
        let mut value = json!({
            "model": model,
            "input": [{"role": "user", "content": [
                {"type": "input_text", "text": "请只回复你看到的像素颜色。"},
                {"type": "input_image", "image_url": CAPABILITY_TEST_IMAGE_DATA_URL}
            ]}],
            "max_output_tokens": 32,
        });
        if !reasoning_effort.is_empty() && reasoning_effort != "none" {
            value["reasoning"] = json!({"effort": reasoning_effort});
        }
        value
    } else {
        json!({
            "model": model,
            "messages": [{"role": "user", "content": [
                {"type": "text", "text": "请只回复你看到的像素颜色。"},
                {"type": "image_url", "image_url": {"url": CAPABILITY_TEST_IMAGE_DATA_URL, "detail": "low"}}
            ]}],
            "max_tokens": 32,
            "temperature": 0,
        })
    };
    let response = post_capability_json(
        client,
        &capability_endpoint(
            base_url,
            if wire_api == "responses" {
                "/responses"
            } else {
                "/chat/completions"
            },
        ),
        keys,
        body,
    )
    .await?;
    if response_has_chat_content(&response, wire_api) {
        Ok(())
    } else {
        Err("视觉接口没有返回可用内容".into())
    }
}

async fn test_media_capability(
    client: &reqwest::Client,
    base_url: &str,
    keys: &[String],
    model: &str,
    capability: ModelCapability,
) -> Result<(), String> {
    let (path, body) = match capability {
        ModelCapability::ImageGeneration => (
            "/images/generations",
            json!({
                "model": model,
                "prompt": capability_prompt(capability),
                "size": "1K",
                "ratio": "1:1",
                "extra_body": {"response_format": "url"},
            }),
        ),
        ModelCapability::ImageEdit => (
            "/images/generations",
            json!({
                "model": model,
                "prompt": capability_prompt(capability),
                "size": "1K",
                "ratio": "1:1",
                "extra_body": {
                    "response_format": "url",
                    "image": [CAPABILITY_TEST_IMAGE_DATA_URL]
                }
            }),
        ),
        ModelCapability::VideoGeneration => (
            "/videos",
            json!({
                "model": model,
                "prompt": capability_prompt(capability),
                "seconds": "1",
                "size": "256x256",
            }),
        ),
        _ => return Err("该能力不属于媒体接口".into()),
    };
    let response =
        post_capability_json(client, &capability_endpoint(base_url, path), keys, body).await?;
    let valid = match capability {
        ModelCapability::VideoGeneration => response.get("id").and_then(Value::as_str).is_some(),
        ModelCapability::ImageGeneration | ModelCapability::ImageEdit => response
            .get("data")
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("上游响应不符合当前能力协议".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_endpoint_supports_v1_and_root_base_urls() {
        assert_eq!(
            capability_endpoint("https://example.test/v1", "/responses"),
            "https://example.test/v1/responses"
        );
        assert_eq!(
            capability_endpoint("https://example.test", "/responses"),
            "https://example.test/v1/responses"
        );
    }

    #[test]
    fn capability_base_strips_known_operation_suffixes() {
        assert_eq!(
            normalize_capability_base_url("https://example.test/v1/chat/completions").unwrap(),
            "https://example.test/v1"
        );
        assert!(normalize_capability_base_url("example.test/v1").is_err());
    }

    #[test]
    fn capability_keys_are_trimmed_and_deduped() {
        assert_eq!(
            unique_api_keys(
                &[" key-a ".into(), "key-a".into(), "".into()],
                Some("key-b"),
            ),
            vec!["key-a", "key-b"]
        );
    }
}
