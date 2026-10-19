use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tracing::debug;

use super::types::*;
use crate::models::{ChatAttachment, ChatMessage, LlmProfileConfig};

static API_KEY_ROUND_ROBIN: Lazy<Mutex<HashMap<String, usize>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn single_key_vec(api_key: &str) -> Vec<String> {
    let key = api_key.trim();
    if key.is_empty() {
        vec![]
    } else {
        vec![key.to_string()]
    }
}

fn profile_api_keys(profile: &LlmProfileConfig) -> Vec<String> {
    let mut keys = Vec::new();
    for key in &profile.api_keys {
        let key = key.trim();
        if !key.is_empty() && !keys.iter().any(|item: &String| item == key) {
            keys.push(key.to_string());
        }
    }
    if let Some(api_key) = profile.api_key.as_deref() {
        let api_key = api_key.trim();
        if !api_key.is_empty() && !keys.iter().any(|item| item == api_key) {
            keys.push(api_key.to_string());
        }
    }
    keys
}

fn rotate_keys(scope: &str, keys: Vec<String>) -> Vec<String> {
    if keys.len() <= 1 {
        return keys;
    }

    let start = API_KEY_ROUND_ROBIN
        .lock()
        .map(|mut cursors| {
            let cursor = cursors.entry(scope.to_string()).or_insert(0);
            let start = *cursor % keys.len();
            *cursor = (*cursor + 1) % keys.len();
            start
        })
        .unwrap_or(0);

    keys.iter()
        .cycle()
        .skip(start)
        .take(keys.len())
        .cloned()
        .collect()
}

fn apply_profile(
    client: &mut LlmClient,
    profile: &LlmProfileConfig,
    preferred_model: Option<&str>,
    active_model: Option<&str>,
    scope: &str,
) {
    if !profile.base_url.trim().is_empty() {
        client.base_url = profile.base_url.trim_end_matches('/').to_string();
    }
    client.wire_api = if profile.wire_api == "responses" {
        "responses".to_string()
    } else {
        "chat_completions".to_string()
    };
    client.reasoning_effort = if profile.reasoning_effort.trim().is_empty() {
        "medium".to_string()
    } else {
        profile.reasoning_effort.trim().to_string()
    };

    let keys = profile_api_keys(profile);
    client.api_keys = rotate_keys(scope, keys);

    let requested_model = preferred_model.filter(|model| {
        profile.models.iter().any(|item| item == *model) && is_chat_compatible_model(model)
    });
    client.model = requested_model
        .map(|item| item.to_string())
        .or_else(|| {
            active_model.and_then(|model| {
                if profile.models.iter().any(|item| item == model)
                    && is_chat_compatible_model(model)
                {
                    Some(model.to_string())
                } else {
                    None
                }
            })
        })
        .or_else(|| {
            if is_chat_compatible_model(&profile.default_model) {
                Some(profile.default_model.clone())
            } else {
                profile
                    .models
                    .iter()
                    .find(|item| is_chat_compatible_model(item))
                    .cloned()
            }
        })
        .unwrap_or_else(|| profile.default_model.clone());
}

fn should_retry_with_next_key(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

pub struct LlmClient {
    http: Client,
    base_url: String,
    api_keys: Vec<String>,
    model: String,
    wire_api: String,
    reasoning_effort: String,
    timeout: Duration,
}

impl LlmClient {
    pub fn new() -> Self {
        let cfg = crate::config::config();
        let http = Client::builder()
            .timeout(Duration::from_millis(cfg.llm_chat_timeout_ms))
            .build()
            .expect("reqwest client build");
        let api_keys = if cfg.llm_api_keys.is_empty() {
            single_key_vec(&cfg.llm_api_key)
        } else {
            cfg.llm_api_keys.clone()
        };
        Self {
            http,
            base_url: cfg.llm_text_base_url.trim_end_matches('/').to_string(),
            api_keys,
            model: cfg.llm_text_model.clone(),
            wire_api: cfg.llm_text_wire_api.clone(),
            reasoning_effort: cfg.llm_text_reasoning_effort.clone(),
            timeout: Duration::from_millis(cfg.llm_chat_timeout_ms),
        }
    }

    pub fn from_profile(
        profile: &LlmProfileConfig,
        preferred_model: Option<&str>,
        scope: &str,
    ) -> Self {
        let mut client = Self::new();
        apply_profile(&mut client, profile, preferred_model, None, scope);
        client
    }

    pub async fn for_user(user_id: &str, preferred_model: Option<&str>) -> Self {
        let mut client = Self::new();
        let pool = crate::state::db_pool();

        let settings = crate::db::settings_repo::find_by_user(&pool, user_id)
            .await
            .ok()
            .flatten();

        if let Some(settings) = settings {
            if let Some(profile) = settings
                .llm_profiles
                .iter()
                .find(|item| item.id == settings.active_profile_id)
            {
                apply_profile(
                    &mut client,
                    profile,
                    preferred_model,
                    Some(settings.active_model.as_str()),
                    &format!("user:{user_id}:profile:{}", profile.id),
                );
            }
        } else if let Some(model) =
            preferred_model.filter(|item| !item.trim().is_empty() && is_chat_compatible_model(item))
        {
            client.model = model.to_string();
        }

        client
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// 非流式 chat（带可选工具）
    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[FunctionDef]>,
    ) -> Result<ChatCompletionResponse> {
        self.chat_with_attachments(messages, tools, None).await
    }

    pub async fn chat_with_attachments(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[FunctionDef]>,
        user_attachments: Option<&[ChatAttachment]>,
    ) -> Result<ChatCompletionResponse> {
        let has_image_attachments = has_image_attachments(user_attachments);
        let req = ChatCompletionRequest {
            model: self.model.clone(),
            messages: build_request_messages(messages, user_attachments),
            tools: tools.map(|t| t.to_vec()),
            tool_choice: None,
            temperature: Some(0.7),
            stream: Some(false),
        };

        match self.send_chat_request(&req).await {
            Ok(result) => Ok(result),
            Err(err) if has_image_attachments && tools.is_some() => {
                tracing::warn!(
                    "vision request with tools failed, retrying vision chat without tools: {err}"
                );
                let vision_only_req = ChatCompletionRequest {
                    model: self.model.clone(),
                    messages: build_request_messages(messages, user_attachments),
                    tools: None,
                    tool_choice: None,
                    temperature: Some(0.7),
                    stream: Some(false),
                };

                match self.send_chat_request(&vision_only_req).await {
                    Ok(result) => Ok(result),
                    Err(vision_err) => {
                        tracing::warn!(
                            "vision-only retry failed, retrying text-only chat: {vision_err}"
                        );
                        self.retry_text_only_after_vision_failure(messages, tools)
                            .await
                    }
                }
            }
            Err(err) if has_image_attachments => {
                tracing::warn!("vision request failed, retrying text-only chat: {err}");
                self.retry_text_only_after_vision_failure(messages, tools)
                    .await
            }
            Err(err) => Err(err),
        }
    }

    async fn retry_text_only_after_vision_failure(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[FunctionDef]>,
    ) -> Result<ChatCompletionResponse> {
        let mut fallback_messages = messages.to_vec();
        fallback_messages.push(ChatMessage {
            role: "system".to_string(),
            content: "当前模型或接口暂未成功处理本次图片视觉输入。不要假装已经看到了图片内容；请明确告知用户当前限制，并优先依据 OCR 文本、图片中的关键区域说明或用户补充描述继续回答。".to_string(),
            tool_calls: None,
            tool_call_id: None,
        });
        let fallback_req = ChatCompletionRequest {
            model: self.model.clone(),
            messages: build_request_messages(&fallback_messages, None),
            tools: tools.map(|t| t.to_vec()),
            tool_choice: None,
            temperature: Some(0.7),
            stream: Some(false),
        };
        self.send_chat_request(&fallback_req).await
    }

    async fn send_chat_request(
        &self,
        req: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        if self.wire_api == "responses" {
            return self.send_responses_request(req).await;
        }
        let url = format!("{}/chat/completions", self.base_url);
        debug!("LLM chat request to {url}");

        let keys = if self.api_keys.is_empty() {
            vec![String::new()]
        } else {
            self.api_keys.clone()
        };
        let mut last_error: Option<anyhow::Error> = None;

        for (index, api_key) in keys.iter().enumerate() {
            let resp = self
                .http
                .post(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .header("Content-Type", "application/json")
                .timeout(self.timeout)
                .json(req)
                .send()
                .await;

            let resp = match resp {
                Ok(resp) => resp,
                Err(err) => {
                    last_error = Some(err.into());
                    if index + 1 < keys.len() {
                        continue;
                    }
                    break;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let err = anyhow!("LLM 返回错误 {status}: {body}");
                if should_retry_with_next_key(status) && index + 1 < keys.len() {
                    tracing::warn!("LLM key failed with {status}, retrying next key");
                    last_error = Some(err);
                    continue;
                }
                return Err(err);
            }

            let result: ChatCompletionResponse = resp.json().await?;
            return Ok(result);
        }

        Err(last_error.unwrap_or_else(|| anyhow!("当前模型服务未配置可用 API Key")))
    }

    async fn send_responses_request(
        &self,
        req: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        let url = format!("{}/responses", self.base_url);
        let mut body = json!({
            "model": req.model.clone(),
            "input": responses_input(&req.messages),
            "stream": false,
        });
        if let Some(tools) = req.tools.as_ref() {
            body["tools"] = serde_json::to_value(responses_tools(tools))?;
        }
        if !self.reasoning_effort.is_empty() && self.reasoning_effort != "none" {
            body["reasoning"] = json!({ "effort": self.reasoning_effort });
        }

        let keys = if self.api_keys.is_empty() {
            vec![String::new()]
        } else {
            self.api_keys.clone()
        };
        let mut last_error: Option<anyhow::Error> = None;

        for (index, api_key) in keys.iter().enumerate() {
            let mut request = self
                .http
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .timeout(self.timeout)
                .json(&body);
            if !api_key.is_empty() {
                request = request.header("Authorization", format!("Bearer {api_key}"));
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(error.into());
                    if index + 1 < keys.len() {
                        continue;
                    }
                    break;
                }
            };

            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            if !status.is_success() {
                let error = anyhow!("Responses API 返回错误 {status}: {text}");
                if should_retry_with_next_key(status) && index + 1 < keys.len() {
                    last_error = Some(error);
                    continue;
                }
                return Err(error);
            }

            let value: Value = serde_json::from_str(&text)
                .map_err(|error| anyhow!("Responses API 返回 JSON 无法解析: {error}"))?;
            return responses_to_chat_response(value, &self.model);
        }

        Err(last_error.unwrap_or_else(|| anyhow!("当前模型服务未配置可用 API Key")))
    }

    /// 提取 JSON（容错：去 markdown fence、截取首尾花括号）
    pub fn extract_json(text: &str) -> Result<serde_json::Value> {
        let cleaned = text
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        if let Ok(v) = serde_json::from_str::<serde_json::Value>(cleaned) {
            return Ok(v);
        }

        // 尝试截取第一个 { 到最后一个 }
        if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
            if end > start {
                if let Ok(v) = serde_json::from_str(&cleaned[start..=end]) {
                    return Ok(v);
                }
            }
        }
        // 尝试数组
        if let (Some(start), Some(end)) = (cleaned.find('['), cleaned.rfind(']')) {
            if end > start {
                if let Ok(v) = serde_json::from_str(&cleaned[start..=end]) {
                    return Ok(v);
                }
            }
        }
        Err(anyhow!("模型未返回可解析 JSON"))
    }
}

fn responses_input(messages: &[RequestMessage]) -> Vec<Value> {
    let mut input = Vec::new();
    for message in messages {
        if message.role == "tool" {
            if let Some(call_id) = message.tool_call_id.as_deref() {
                input.push(json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": message.content.clone(),
                }));
            }
            continue;
        }

        let content_text = message
            .content
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| message.content.to_string());
        if !content_text.is_empty() {
            let content_type = if message.role == "assistant" {
                "output_text"
            } else {
                "input_text"
            };
            input.push(json!({
                "role": message.role,
                "content": [{"type": content_type, "text": content_text}],
            }));
        }

        if let Some(tool_calls) = &message.tool_calls {
            for call in tool_calls {
                let function = call.get("function").cloned().unwrap_or_default();
                let call_id = call.get("id").and_then(Value::as_str).unwrap_or_default();
                let name = function
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let arguments = function
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}");
                input.push(json!({
                    "type": "function_call",
                    "call_id": call_id,
                    "name": name,
                    "arguments": arguments,
                }));
            }
        }
    }
    input
}

fn responses_tools(tools: &[FunctionDef]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "name": tool.function.name,
                "description": tool.function.description,
                "parameters": tool.function.parameters,
            })
        })
        .collect()
}

fn responses_to_chat_response(
    value: Value,
    fallback_model: &str,
) -> Result<ChatCompletionResponse> {
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    if let Some(output) = value.get("output").and_then(Value::as_array) {
        for item in output {
            match item.get("type").and_then(Value::as_str).unwrap_or_default() {
                "message" => {
                    if let Some(parts) = item.get("content").and_then(Value::as_array) {
                        for part in parts {
                            if let Some(text) = part.get("text").and_then(Value::as_str) {
                                content.push_str(text);
                            }
                        }
                    }
                }
                "function_call" => {
                    let call_id = item
                        .get("call_id")
                        .or_else(|| item.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let arguments = item
                        .get("arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("{}")
                        .to_string();
                    tool_calls.push(ToolCall {
                        id: call_id,
                        call_type: "function".into(),
                        function: ToolFunction { name, arguments },
                    });
                }
                _ => {}
            }
        }
    }
    if content.is_empty() {
        content = value
            .get("output_text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
    }
    Ok(ChatCompletionResponse {
        choices: vec![Choice {
            message: ResponseMessage {
                role: "assistant".into(),
                content: if content.is_empty() {
                    None
                } else {
                    Some(content)
                },
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                },
            },
            finish_reason: Some("stop".into()),
        }],
        model: value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(fallback_model)
            .to_string(),
    })
}

fn build_request_messages(
    messages: &[ChatMessage],
    user_attachments: Option<&[ChatAttachment]>,
) -> Vec<RequestMessage> {
    let latest_user_index = user_attachments
        .filter(|items| has_image_attachments(Some(items)))
        .and_then(|_| messages.iter().rposition(|msg| msg.role == "user"));

    messages
        .iter()
        .enumerate()
        .map(
            |(index, msg)| match (Some(index) == latest_user_index, user_attachments) {
                (true, Some(attachments)) => {
                    RequestMessage::from_multimodal_user_message(msg, attachments)
                }
                _ => RequestMessage::from_chat_message(msg),
            },
        )
        .collect()
}

fn has_image_attachments(user_attachments: Option<&[ChatAttachment]>) -> bool {
    user_attachments
        .map(|items| {
            items.iter().any(|item| {
                item.kind == "image"
                    && item
                        .data_url
                        .as_deref()
                        .map(|value| !value.trim().is_empty())
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn is_chat_compatible_model(model: &str) -> bool {
    let normalized = model.trim().to_lowercase();
    !(normalized.starts_with("agnes-image-")
        || normalized.starts_with("agnes-video-")
        || normalized.starts_with("gpt-image-")
        || normalized.starts_with("gpt-video-")
        || normalized.starts_with("image-")
        || normalized.starts_with("video-"))
}
