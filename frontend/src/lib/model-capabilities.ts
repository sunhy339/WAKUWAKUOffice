import type { LLMProfile, ModelCapability } from '@/types'

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
