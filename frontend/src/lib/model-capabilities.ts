import type { LLMModelConfig, LLMProfile, ModelCapability } from '@/types'

const chatCapabilities: ModelCapability[] = ['text', 'tool_calling', 'vision']

export function legacyModelCapabilities(model: string): ModelCapability[] {
  const normalized = model.trim().toLowerCase()
  if (/^(agnes|gpt)-image-/.test(normalized) || /^image-/.test(normalized)) {
    return ['image_generation', 'image_edit']
  }
  if (/^(agnes|gpt)-video-/.test(normalized) || /^video-/.test(normalized)) {
    return ['video_generation']
  }
  return ['text', 'tool_calling', 'vision']
}

export function modelCapabilities(profile: LLMProfile, model: string): ModelCapability[] {
  return profile.model_configs?.find((config) => config.model === model)?.capabilities
    || legacyModelCapabilities(model)
}

export function effectiveModelConfigs(profile: LLMProfile): LLMModelConfig[] {
  const configured = Array.isArray(profile.model_configs) ? profile.model_configs : []
  const models = Array.from(new Set([...(profile.models || []), ...configured.map((item) => item.model)]))
  return models.map((model) => configured.find((item) => item.model === model) || ({
    model,
    capabilities: legacyModelCapabilities(model),
    enabled: true,
    priority: 0,
    status: 'unknown',
  }))
}

function isMediaModelForChat(model: string, capability: ModelCapability) {
  if (!['text', 'tool_calling', 'vision'].includes(capability)) return false
  const normalized = model.trim().toLowerCase()
  return /^(agnes|gpt)-(image|video)-/.test(normalized) || /^(image|video)-/.test(normalized)
}

/**
 * 使用与服务端一致的候选顺序计算设置页预览：当前模型 → 服务默认模型 → 优先级。
 * 这里只返回非敏感模型名，真正执行仍由服务端再次路由和校验。
 */
export function routeModelsForCapability(
  profile: LLMProfile,
  preferredModel: string | undefined,
  capability: ModelCapability,
): string[] {
  const configs = effectiveModelConfigs(profile)
  const eligible = (config: LLMModelConfig) => config.enabled
    && config.status !== 'unavailable'
    && config.capability_statuses?.[capability] !== 'unavailable'
    && config.capabilities.includes(capability)
    && !isMediaModelForChat(config.model, capability)
  const selected: string[] = []
  const append = (model?: string) => {
    const value = model?.trim()
    if (!value) return
    const config = configs.find((item) => item.model === value)
    if (config && eligible(config) && !selected.includes(config.model)) selected.push(config.model)
  }
  append(preferredModel)
  append(profile.default_model)
  configs
    .filter(eligible)
    .sort((left, right) => left.priority - right.priority || left.model.localeCompare(right.model))
    .forEach((config) => append(config.model))
  return selected
}

export function supportsChatModel(profile: LLMProfile, model: string): boolean {
  const normalized = model.trim().toLowerCase()
  if (/^(agnes|gpt)-(image|video)-/.test(normalized) || /^(image|video)-/.test(normalized)) {
    return false
  }
  return modelCapabilities(profile, model).some((capability) => chatCapabilities.includes(capability))
}

export function chatModelsFromProfiles(profiles: LLMProfile[]): string[] {
  return Array.from(new Set(
    profiles.flatMap((profile) => (profile.models || []).filter((model) => supportsChatModel(profile, model))),
  ))
}
