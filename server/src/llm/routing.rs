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
    select_models(profile, preferred_model, requirement)
        .into_iter()
        .next()
}

/// 按能力返回所有可用候选模型。
///
/// 候选顺序是稳定的：显式选择的模型优先，其次是配置默认模型，最后按
/// priority（数值越小越优先）和模型名排序。调用方可以在上游返回可重试
/// 错误或响应不符合协议时尝试下一个模型，而不需要复制能力筛选规则。
pub fn select_models(
    profile: &LlmProfileConfig,
    preferred_model: Option<&str>,
    requirement: ModelRequirement,
) -> Vec<ModelRoute> {
    let configs = effective_model_configs(profile);
    let capability = requirement.capability();

    let eligible = |config: &LlmModelConfig| {
        config.enabled
            && config.status != ModelAvailability::Unavailable
            && config
                .capability_statuses
                .get(&capability)
                .copied()
                .unwrap_or(config.status)
                != ModelAvailability::Unavailable
            && config.capabilities.contains(&capability)
            && !is_media_model_for_text(config.model.as_str(), requirement)
    };

    let mut selected = Vec::new();
    let mut append_if_eligible = |model: Option<&str>| {
        let Some(model) = model.map(str::trim).filter(|item| !item.is_empty()) else {
            return;
        };
        let Some(config) = configs
            .iter()
            .find(|config| config.model == model && eligible(config))
        else {
            return;
        };
        if !selected
            .iter()
            .any(|route: &ModelRoute| route.model == config.model)
        {
            selected.push(route_from_config(profile, config));
        }
    };

    append_if_eligible(preferred_model);
    append_if_eligible(Some(profile.default_model.as_str()));

    let mut remaining = configs
        .iter()
        .filter(|config| eligible(config))
        .collect::<Vec<_>>();
    remaining.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.model.to_lowercase().cmp(&right.model.to_lowercase()))
    });
    for config in remaining {
        if !selected
            .iter()
            .any(|route: &ModelRoute| route.model == config.model)
        {
            selected.push(route_from_config(profile, config));
        }
    }

    selected
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
        capability_statuses: std::collections::HashMap::new(),
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
            capability_statuses: std::collections::HashMap::new(),
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
    fn select_models_keeps_preferred_and_default_before_priority_fallbacks() {
        let profile = LlmProfileConfig {
            id: "profile".into(),
            name: "测试服务".into(),
            base_url: "https://example.test/v1".into(),
            provider: "OpenAI Compatible".into(),
            wire_api: "responses".into(),
            reasoning_effort: "medium".into(),
            api_keys: vec![],
            models: vec![
                "backup-low".into(),
                "preferred".into(),
                "default".into(),
                "backup-high".into(),
            ],
            default_model: "default".into(),
            model_configs: vec![
                model("backup-low", vec![ModelCapability::ImageGeneration], 10),
                model("preferred", vec![ModelCapability::ImageGeneration], 99),
                model("default", vec![ModelCapability::ImageGeneration], 50),
                model("backup-high", vec![ModelCapability::ImageGeneration], 1),
            ],
            api_key: None,
            has_api_key: false,
        };

        let routes = select_models(
            &profile,
            Some("preferred"),
            ModelRequirement::ImageGeneration,
        );

        assert_eq!(
            routes
                .into_iter()
                .map(|route| route.model)
                .collect::<Vec<_>>(),
            vec!["preferred", "default", "backup-high", "backup-low"]
        );
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
    fn capability_unavailability_only_skips_that_capability() {
        let mut partially_available = model(
            "mixed-model",
            vec![ModelCapability::Text, ModelCapability::ImageGeneration],
            0,
        );
        partially_available.capability_statuses.insert(
            ModelCapability::ImageGeneration,
            ModelAvailability::Unavailable,
        );
        let profile = profile(vec![partially_available]);

        assert_eq!(
            select_model(&profile, None, ModelRequirement::Text)
                .unwrap()
                .model,
            "mixed-model"
        );
        assert!(select_model(&profile, None, ModelRequirement::ImageGeneration).is_none());
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
