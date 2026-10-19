use crate::models::{LlmModelConfig, LlmProfileConfig, ModelAvailability, ModelCapability};

/// 一次模型调用对能力的最低要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRequirement {
    Text,
    ToolCalling,
    Vision,
    ImageGeneration,
    ImageEdit,
    VideoGeneration,
}

impl ModelRequirement {
    pub fn capability(self) -> ModelCapability {
        match self {
            Self::Text => ModelCapability::Text,
            Self::ToolCalling => ModelCapability::ToolCalling,
            Self::Vision => ModelCapability::Vision,
            Self::ImageGeneration => ModelCapability::ImageGeneration,
            Self::ImageEdit => ModelCapability::ImageEdit,
            Self::VideoGeneration => ModelCapability::VideoGeneration,
        }
    }
}

/// 为一次调用选出的模型。路由层只返回非敏感配置，不携带 API Key。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRoute {
    pub profile_id: String,
    pub model: String,
    pub capabilities: Vec<ModelCapability>,
    pub status: ModelAvailability,
}

/// 返回配置中可用于路由的模型。
///
/// 旧版本只有 `models` 字段。对于这类配置，使用兼容推导：普通模型至少
/// 保留现有的文本、工具和视觉对话能力；明显的图片/视频模型只保留媒体能力，
/// 防止它们被误选为普通对话模型。
pub fn effective_model_configs(profile: &LlmProfileConfig) -> Vec<LlmModelConfig> {
    let mut configs = Vec::new();

    for model in &profile.models {
        if let Some(config) = profile
            .model_configs
            .iter()
            .find(|item| item.model == *model)
        {
            configs.push(config.clone());
        } else {
            configs.push(legacy_model_config(model));
        }
    }

    // 兼容已经保存了 model_configs、但模型列表尚未同步的中间版本。
    for config in &profile.model_configs {
        if !configs.iter().any(|item| item.model == config.model) {
            configs.push(config.clone());
        }
    }

    configs
}

/// 按能力选择模型。优先级数值越小越优先；preferred_model 是用户当前显式选择。
pub fn select_model(
    profile: &LlmProfileConfig,
    preferred_model: Option<&str>,
    requirement: ModelRequirement,
) -> Option<ModelRoute> {
    let configs = effective_model_configs(profile);
    let capability = requirement.capability();

    let eligible = |config: &LlmModelConfig| {
        config.enabled
            && config.status != ModelAvailability::Unavailable
            && config.capabilities.contains(&capability)
            && !is_media_model_for_text(config.model.as_str(), requirement)
    };

    if let Some(preferred) = preferred_model
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        if let Some(config) = configs
            .iter()
            .find(|config| config.model == preferred && eligible(config))
        {
            return Some(route_from_config(profile, config));
        }
    }

    if let Some(config) = configs
        .iter()
        .find(|config| config.model == profile.default_model && eligible(config))
    {
        return Some(route_from_config(profile, config));
    }

    configs
        .iter()
        .filter(|config| eligible(config))
        .min_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.model.to_lowercase().cmp(&right.model.to_lowercase()))
        })
        .map(|config| route_from_config(profile, config))
}

pub fn legacy_model_config(model: &str) -> LlmModelConfig {
    let normalized = model.trim().to_lowercase();
    let capabilities = if is_image_model(&normalized) {
        vec![ModelCapability::ImageGeneration, ModelCapability::ImageEdit]
    } else if is_video_model(&normalized) {
        vec![ModelCapability::VideoGeneration]
    } else {
        // 旧版本已经把图片附件交给当前对话模型，因此把视觉能力纳入
        // 兼容推导，避免启用路由后破坏已有的识图入口。
        vec![
            ModelCapability::Text,
            ModelCapability::ToolCalling,
            ModelCapability::Vision,
        ]
    };

    LlmModelConfig {
        model: model.trim().to_string(),
        capabilities,
        enabled: true,
        priority: 0,
        status: ModelAvailability::Unknown,
        last_checked_at: None,
        last_error: None,
    }
}

fn route_from_config(profile: &LlmProfileConfig, config: &LlmModelConfig) -> ModelRoute {
    ModelRoute {
        profile_id: profile.id.clone(),
        model: config.model.clone(),
        capabilities: config.capabilities.clone(),
        status: config.status,
    }
}

fn is_media_model_for_text(model: &str, requirement: ModelRequirement) -> bool {
    matches!(
        requirement,
        ModelRequirement::Text | ModelRequirement::ToolCalling | ModelRequirement::Vision
    ) && (is_image_model(model) || is_video_model(model))
}

fn is_image_model(model: &str) -> bool {
    model.starts_with("agnes-image-")
        || model.starts_with("gpt-image-")
        || model.starts_with("image-")
}

fn is_video_model(model: &str) -> bool {
    model.starts_with("agnes-video-")
        || model.starts_with("gpt-video-")
        || model.starts_with("video-")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(models: Vec<LlmModelConfig>) -> LlmProfileConfig {
        LlmProfileConfig {
            id: "profile".into(),
            name: "测试服务".into(),
            base_url: "https://example.test/v1".into(),
            provider: "OpenAI Compatible".into(),
            wire_api: "responses".into(),
            reasoning_effort: "medium".into(),
            api_keys: vec![],
            models: models.iter().map(|item| item.model.clone()).collect(),
            default_model: models
                .first()
                .map(|item| item.model.clone())
                .unwrap_or_default(),
            model_configs: models,
            api_key: None,
            has_api_key: false,
        }
    }

    fn model(name: &str, capabilities: Vec<ModelCapability>, priority: i32) -> LlmModelConfig {
        LlmModelConfig {
            model: name.into(),
            capabilities,
            enabled: true,
            priority,
            status: ModelAvailability::Unknown,
            last_checked_at: None,
            last_error: None,
        }
    }

    #[test]
    fn preferred_model_must_have_required_capability() {
        let profile = profile(vec![
            model("text-model", vec![ModelCapability::Text], 0),
            model("image-model", vec![ModelCapability::ImageGeneration], 0),
        ]);

        let route = select_model(
            &profile,
            Some("text-model"),
            ModelRequirement::ImageGeneration,
        )
        .unwrap();

        assert_eq!(route.model, "image-model");
    }

    #[test]
    fn unavailable_model_is_skipped() {
        let mut unavailable = model("primary", vec![ModelCapability::Text], 0);
        unavailable.status = ModelAvailability::Unavailable;
        let profile = profile(vec![
            unavailable,
            model("backup", vec![ModelCapability::Text], 1),
        ]);

        assert_eq!(
            select_model(&profile, None, ModelRequirement::Text)
                .unwrap()
                .model,
            "backup"
        );
    }

    #[test]
    fn legacy_media_names_are_not_chat_models() {
        let profile = LlmProfileConfig {
            id: "legacy".into(),
            name: "旧服务".into(),
            base_url: "https://example.test/v1".into(),
            provider: "OpenAI Compatible".into(),
            wire_api: "chat_completions".into(),
            reasoning_effort: "medium".into(),
            api_keys: vec![],
            models: vec!["gpt-image-2.5".into(), "legacy-text".into()],
            default_model: "gpt-image-2.5".into(),
            model_configs: vec![],
            api_key: None,
            has_api_key: false,
        };

        assert_eq!(
            select_model(&profile, None, ModelRequirement::Text)
                .unwrap()
                .model,
            "legacy-text"
        );
        assert_eq!(
            select_model(&profile, None, ModelRequirement::ImageGeneration)
                .unwrap()
                .model,
            "gpt-image-2.5"
        );
    }
}
