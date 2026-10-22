import { Check, Cpu, Eye, EyeOff, FlaskConical, Image, KeyRound, Layers3, MessageSquare, Plus, RefreshCw, ScanSearch, SlidersHorizontal, Trash2, Video, X } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { settingsApi } from '@/api'
import type { AppSettings, LLMModelConfig, LLMProfile, MCPServiceConfig, ModelCapability } from '@/types'
import { routeModelsForCapability } from '@/lib/model-capabilities'

interface SettingsDialogProps {
  open: boolean
  settings: AppSettings | null
  onClose: () => void
  onSave: (settings: AppSettings) => Promise<void> | void
}

const emptyProfile = (): LLMProfile => ({
  id: `profile-${Date.now()}`,
  name: '新的模型服务',
  base_url: 'http://127.0.0.1:8777/v1',
  provider: 'OpenAI Compatible',
  wire_api: 'chat_completions',
  reasoning_effort: 'medium',
  api_key: '',
  api_keys: [],
  models: [],
  default_model: '',
  model_configs: [],
})

const capabilityOptions: Array<{ value: ModelCapability; label: string }> = [
  { value: 'text', label: '文本' },
  { value: 'tool_calling', label: '工具' },
  { value: 'vision', label: '视觉' },
  { value: 'image_generation', label: '文生图' },
  { value: 'image_edit', label: '图生图' },
  { value: 'video_generation', label: '视频' },
]

const conversationCapabilities: ModelCapability[] = ['text', 'tool_calling', 'vision']
const mediaCapabilities: ModelCapability[] = ['image_generation', 'image_edit', 'video_generation']

function modelStatusForCapabilities(config: LLMModelConfig, statuses: LLMModelConfig['capability_statuses']): LLMModelConfig['status'] {
  const current = config.capabilities.map((capability) => statuses?.[capability])
  if (current.length > 0 && current.every((status) => status === 'unavailable')) return 'unavailable'
  if (current.some((status) => status === 'available')) return 'available'
  return 'unknown'
}

const routePreviewOptions: Array<{ capability: ModelCapability; label: string; description: string; Icon: typeof MessageSquare }> = [
  { capability: 'text', label: '普通对话', description: '文本回答与日常交流', Icon: MessageSquare },
  { capability: 'vision', label: '识图分析', description: '图片理解、OCR 与截图分析', Icon: ScanSearch },
  { capability: 'image_generation', label: '文生图', description: '根据描述生成图片', Icon: Image },
  { capability: 'image_edit', label: '图生图', description: '使用参考图进行编辑', Icon: Layers3 },
  { capability: 'video_generation', label: '视频生成', description: '文生视频与图生视频', Icon: Video },
]

function legacyModelCapabilities(model: string): ModelCapability[] {
  const normalized = model.trim().toLowerCase()
  if (/^(agnes|gpt)-image-/.test(normalized) || /^image-/.test(normalized)) {
    return ['image_generation', 'image_edit']
  }
  if (/^(agnes|gpt)-video-/.test(normalized) || /^video-/.test(normalized)) {
    return ['video_generation']
  }
  return ['text', 'tool_calling', 'vision']
}

function getModelConfigs(profile: LLMProfile): LLMModelConfig[] {
  const configured = Array.isArray(profile.model_configs) ? profile.model_configs : []
  return (profile.models || []).map((model) => configured.find((item) => item.model === model) || ({
    model,
    capabilities: legacyModelCapabilities(model),
    enabled: true,
    priority: 0,
    status: 'unknown',
    capability_statuses: {},
  }))
}

function getProfileKeys(profile: LLMProfile) {
  const keys = Array.isArray(profile.api_keys) ? profile.api_keys : []
  const merged = [...keys, profile.api_key || '']
    .map((item) => item.trim())
    .filter(Boolean)
  return Array.from(new Set(merged))
}

function maskApiKeys(keys: string[]) {
  return keys.map(() => '••••••••').join('\n')
}

const emptyMcpServer = (): MCPServiceConfig => ({
  id: `mcp-${Date.now()}`,
  name: '新的 MCP 服务',
  transport: 'http',
  endpoint: 'http://127.0.0.1:3001',
  enabled: true,
  description: '',
})

export function SettingsDialog({ open, settings, onClose, onSave }: SettingsDialogProps) {
  const [section, setSection] = useState<'llm' | 'base' | 'mcp'>('llm')
  const [draft, setDraft] = useState<AppSettings | null>(settings)
  const [saving, setSaving] = useState(false)
  const [fetchingModelsId, setFetchingModelsId] = useState<string | null>(null)
  const [modelFetchResults, setModelFetchResults] = useState<Record<string, { ok: boolean; message: string }>>({})
  const [testingLlmId, setTestingLlmId] = useState<string | null>(null)
  const [llmTestResults, setLlmTestResults] = useState<Record<string, { ok: boolean; message: string }>>({})
  const [testingCapabilityKey, setTestingCapabilityKey] = useState<string | null>(null)
  const [capabilityTestResults, setCapabilityTestResults] = useState<Record<string, {
    ok: boolean
    message: string
    results?: Array<{ capability: string; ok: boolean; message: string }>
  }>>({})
  const [visibleApiKeyProfiles, setVisibleApiKeyProfiles] = useState<Record<string, boolean>>({})
  const [testingMcpId, setTestingMcpId] = useState<string | null>(null)
  const [mcpTestResults, setMcpTestResults] = useState<Record<string, { ok: boolean; message: string; tools: string[] }>>({})

  useEffect(() => {
    setDraft(settings)
  }, [settings])

  useEffect(() => {
    if (open) setSection('llm')
  }, [open])

  const hasChanges = useMemo(() => JSON.stringify(draft) !== JSON.stringify(settings), [draft, settings])

  if (!open || !draft) return null

  const updateDraft = (patch: Partial<AppSettings>) => {
    setDraft((prev) => (prev ? { ...prev, ...patch } : prev))
  }

  const updateProfile = (id: string, patch: Partial<LLMProfile>) => {
    const nextProfiles = draft.llm_profiles.map((profile) => {
      if (profile.id !== id) return profile
      const next = { ...profile, ...patch }
      const existingConfigs = Array.isArray(next.model_configs) ? next.model_configs : []
      next.model_configs = (next.models || []).map((model) => existingConfigs.find((item) => item.model === model) || ({
        model,
        capabilities: legacyModelCapabilities(model),
        enabled: true,
        priority: 0,
        status: 'unknown',
        capability_statuses: {},
      }))
      if (!next.models.includes(next.default_model)) {
        next.default_model = next.models[0] || ''
      }
      return next
    })
    updateDraft({ llm_profiles: nextProfiles })
  }

  const removeProfile = (id: string) => {
    if (draft.llm_profiles.length <= 1) return
    const nextProfiles = draft.llm_profiles.filter((profile) => profile.id !== id)
    const activeProfile = nextProfiles.find((profile) => profile.id === draft.active_profile_id) || nextProfiles[0]
    updateDraft({
      llm_profiles: nextProfiles,
      active_profile_id: activeProfile.id,
      default_model: activeProfile.default_model,
      active_model: activeProfile.default_model,
    })
  }

  const addProfile = () => {
    const profile = emptyProfile()
    updateDraft({
      llm_profiles: [...draft.llm_profiles, profile],
      active_profile_id: profile.id,
      default_model: profile.default_model,
      active_model: profile.default_model,
    })
  }

  const setActiveProfile = (profileId: string) => {
    const profile = draft.llm_profiles.find((item) => item.id === profileId)
    if (!profile) return
    updateDraft({
      active_profile_id: profileId,
      default_model: profile.default_model,
      active_model: profile.default_model,
    })
  }

  const updateMcp = (id: string, patch: Partial<MCPServiceConfig>) => {
    updateDraft({
      mcp_servers: draft.mcp_servers.map((server) => (server.id === id ? { ...server, ...patch } : server)),
    })
  }

  const addMcp = () => updateDraft({ mcp_servers: [...draft.mcp_servers, emptyMcpServer()] })

  const removeMcp = (id: string) => updateDraft({ mcp_servers: draft.mcp_servers.filter((server) => server.id !== id) })

  const updateModelConfig = (profile: LLMProfile, model: string, patch: Partial<LLMModelConfig>) => {
    const configs = getModelConfigs(profile).map((config) => (
      config.model === model ? { ...config, ...patch } : config
    ))
    updateProfile(profile.id, { model_configs: configs })
  }

  const toggleModelCapability = (profile: LLMProfile, model: string, capability: ModelCapability) => {
    const config = getModelConfigs(profile).find((item) => item.model === model)
    if (!config) return
    const capabilities = config.capabilities.includes(capability)
      ? config.capabilities.filter((item) => item !== capability)
      : [...config.capabilities, capability]
    const capabilityStatuses = { ...(config.capability_statuses || {}) }
    delete capabilityStatuses[capability]
    updateModelConfig(profile, model, { capabilities, capability_statuses: capabilityStatuses })
  }

  const fetchModels = async (profile: LLMProfile) => {
    setFetchingModelsId(profile.id)
    setModelFetchResults((prev) => ({ ...prev, [profile.id]: { ok: false, message: '正在获取模型列表...' } }))
    try {
      const res = await settingsApi.fetchModels({ base_url: profile.base_url, api_keys: getProfileKeys(profile) })
      const models = Array.isArray(res.data?.models) ? res.data.models : []
      const currentModels = profile.models || []
      const mergedModels = Array.from(new Set([...currentModels, ...models]))
      const nextDefault = profile.default_model && mergedModels.includes(profile.default_model)
        ? profile.default_model
        : mergedModels[0] || ''
      updateProfile(profile.id, { models: mergedModels, default_model: nextDefault })
      if (profile.id === draft.active_profile_id) {
        updateDraft({ active_model: nextDefault, default_model: nextDefault })
      }
      setModelFetchResults((prev) => ({
        ...prev,
        [profile.id]: { ok: true, message: `已获取 ${models.length} 个模型，可在下方调整后保存` },
      }))
    } catch (err: any) {
      setModelFetchResults((prev) => ({
        ...prev,
        [profile.id]: { ok: false, message: err.response?.data?.detail || err.message || '获取模型列表失败，可手动填写' },
      }))
    } finally {
      setFetchingModelsId(null)
    }
  }

  const testLlm = async (profile: LLMProfile) => {
    const model = profile.id === draft.active_profile_id && draft.active_model
      ? draft.active_model
      : profile.default_model || profile.models[0] || ''
    setTestingLlmId(profile.id)
    setLlmTestResults((prev) => ({ ...prev, [profile.id]: { ok: false, message: `正在测试 ${model || '当前模型'}...` } }))
    try {
      const res = await settingsApi.testLlm({
        base_url: profile.base_url,
        api_keys: getProfileKeys(profile),
        model,
        wire_api: profile.wire_api,
        reasoning_effort: profile.reasoning_effort,
      })
      setLlmTestResults((prev) => ({
        ...prev,
        [profile.id]: { ok: !!res.data?.ok, message: res.data?.message || '测试完成' },
      }))
    } catch (err: any) {
      setLlmTestResults((prev) => ({
        ...prev,
        [profile.id]: { ok: false, message: err.response?.data?.detail || err.message || '连接测试失败' },
      }))
    } finally {
      setTestingLlmId(null)
    }
  }

  const testModelCapabilities = async (profile: LLMProfile, config: LLMModelConfig, scope: 'conversation' | 'media') => {
    const requestedCapabilities = config.capabilities.filter((capability) => (
      scope === 'conversation' ? conversationCapabilities : mediaCapabilities
    ).includes(capability))
    const testKey = `${profile.id}:${config.model}:${scope}`
    if (requestedCapabilities.length === 0) {
      setCapabilityTestResults((prev) => ({
        ...prev,
        [testKey]: { ok: false, message: scope === 'conversation' ? '请先勾选文本、工具或视觉能力' : '请先勾选文生图、图生图或视频能力' },
      }))
      return
    }
    setTestingCapabilityKey(testKey)
    setCapabilityTestResults((prev) => ({
      ...prev,
      [testKey]: { ok: false, message: scope === 'media' ? '正在测试媒体能力；本次会调用真实生成接口...' : '正在测试对话能力；发送最小请求...' },
    }))
    try {
      const res = await settingsApi.testModelCapabilities({
        base_url: profile.base_url,
        api_keys: getProfileKeys(profile),
        model: config.model,
        wire_api: profile.wire_api,
        reasoning_effort: profile.reasoning_effort,
        capabilities: requestedCapabilities,
      })
      const results = Array.isArray(res.data?.results) ? res.data.results : []
      const failed = results.filter((item) => !item.ok)
      const capabilityStatuses = { ...(config.capability_statuses || {}) }
      results.forEach((item) => {
        capabilityStatuses[item.capability as ModelCapability] = item.ok ? 'available' : 'unavailable'
      })
      const summary = failed.length === 0
        ? `测试通过：${results.length} 项能力可用`
        : `测试完成：${failed.length} 项失败；${failed.map((item) => item.message).join('；')}`
      updateModelConfig(profile, config.model, {
        status: modelStatusForCapabilities(config, capabilityStatuses),
        capability_statuses: capabilityStatuses,
        last_checked_at: new Date().toISOString(),
        last_error: failed.length > 0 ? failed.map((item) => item.message).join('；') : undefined,
      })
      setCapabilityTestResults((prev) => ({
        ...prev,
        [testKey]: { ok: failed.length === 0 && results.length > 0, message: summary, results },
      }))
    } catch (err: any) {
      const message = err.response?.data?.detail || err.message || '能力测试失败'
      updateModelConfig(profile, config.model, {
        status: modelStatusForCapabilities(config, {
          ...(config.capability_statuses || {}),
          ...Object.fromEntries(requestedCapabilities.map((capability) => [capability, 'unavailable'])),
        } as LLMModelConfig['capability_statuses']),
        capability_statuses: {
          ...(config.capability_statuses || {}),
          ...Object.fromEntries(requestedCapabilities.map((capability) => [capability, 'unavailable'])),
        } as LLMModelConfig['capability_statuses'],
        last_checked_at: new Date().toISOString(),
        last_error: message,
      })
      setCapabilityTestResults((prev) => ({
        ...prev,
        [testKey]: { ok: false, message },
      }))
    } finally {
      setTestingCapabilityKey(null)
    }
  }

  const testMcp = async (server: MCPServiceConfig) => {
    setTestingMcpId(server.id)
    try {
      const res = await settingsApi.testMcp(server)
      const tools = Array.isArray(res.data?.tools)
        ? res.data.tools.map((item: any) => item?.name).filter(Boolean)
        : []
      setMcpTestResults((prev) => ({
        ...prev,
        [server.id]: {
          ok: !!res.data?.ok,
          message: res.data?.message || '测试完成',
          tools,
        },
      }))
    } catch (err: any) {
      setMcpTestResults((prev) => ({
        ...prev,
        [server.id]: {
          ok: false,
          message: err.response?.data?.detail || err.message || '测试失败',
          tools: [],
        },
      }))
    } finally {
      setTestingMcpId(null)
    }
  }

  const handleSave = async () => {
    if (!draft) return
    setSaving(true)
    try {
      await onSave({
        ...draft,
        updated_at: new Date().toISOString(),
      })
    } finally {
      setSaving(false)
    }
  }

  const activeProfile = draft.llm_profiles.find((profile) => profile.id === draft.active_profile_id) || draft.llm_profiles[0]
  const routePreview = activeProfile
    ? routePreviewOptions.map((item) => ({
      ...item,
      models: routeModelsForCapability(activeProfile, draft.active_model, item.capability),
    }))
    : []

  return (
    <div className="h-full overflow-hidden bg-transparent p-5">
      <div className="mx-auto flex h-full max-w-6xl overflow-hidden rounded-[2rem] border border-black/[0.06] bg-white/72 shadow-[0_24px_80px_rgba(24,24,27,0.10)] backdrop-blur-2xl">
        <aside className="w-56 shrink-0 border-r border-black/[0.06] bg-[#eee9df]/70 p-4">
          <div className="mb-5 flex items-center justify-between">
            <div>
              <div className="text-lg font-bold tracking-tight text-surface-950">设置</div>
            </div>
            <button onClick={onClose} className="rounded-full bg-white/75 p-2 text-surface-500 hover:bg-white hover:text-surface-950" title="关闭">
              <X className="h-4 w-4" />
            </button>
          </div>

          <div className="space-y-1.5">
            {[
              ['llm', '模型服务', Cpu],
              ['base', '基础信息', SlidersHorizontal],
              ['mcp', 'MCP 服务', KeyRound],
            ].map(([key, label, Icon]: any) => (
              <button
                key={key}
                type="button"
                onClick={() => setSection(key)}
                className={`flex w-full items-center gap-2 rounded-2xl px-3 py-2.5 text-left text-sm font-semibold transition-all ${section === key ? 'bg-surface-950 text-white shadow-sm' : 'text-surface-600 hover:bg-white/75 hover:text-surface-950'}`}
              >
                <Icon className="h-4 w-4" />
                {label}
              </button>
            ))}
          </div>
        </aside>

        <main className="min-w-0 flex-1 overflow-y-auto p-6">
          {section === 'llm' && (
            <div>
              <div className="mb-5 flex items-start justify-between gap-4">
                <div>
                  <h2 className="text-xl font-bold tracking-tight text-surface-950">模型服务</h2>
                  <p className="mt-1 text-sm text-surface-500">配置多个模型服务，选择默认启用的模型。</p>
                </div>
                <button onClick={addProfile} className="inline-flex items-center gap-1.5 rounded-full bg-surface-950 px-4 py-2 text-sm font-semibold text-white hover:bg-surface-800">
                  <Plus className="h-4 w-4" />
                  添加配置
                </button>
              </div>

              <div className="mb-5 rounded-[1.5rem] border border-blue-200/70 bg-blue-50/70 p-4 text-sm text-blue-900">
                <div className="font-semibold">本地用户怎么使用</div>
                <p className="mt-1 leading-6 text-blue-800/80">
                  在这里填写 OpenAI 兼容服务的 Base URL、API Key 和模型名，保存后即可用于对话。每行一个 Key，服务端会按请求轮询，并在限流或鉴权失败时尝试下一个。配置保存在本机服务端的 SQLite 数据库中，不写入浏览器 localStorage；请不要把数据目录分享给他人。
                </p>
              </div>

              <div className="mb-5 rounded-[1.5rem] border border-black/[0.06] bg-[#f8f5ee]/80 p-4">
                <label className="mb-2 block text-xs font-bold uppercase tracking-[0.14em] text-surface-500">当前默认模型</label>
                <div className="grid gap-3 md:grid-cols-2">
                  <select
                    value={draft.active_profile_id}
                    onChange={(event) => setActiveProfile(event.target.value)}
                    className="rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm outline-none focus:border-surface-500"
                  >
                    {draft.llm_profiles.map((profile) => (
                      <option key={profile.id} value={profile.id}>{profile.name}</option>
                    ))}
                  </select>
                  <select
                    value={draft.active_model}
                    onChange={(event) => updateDraft({ active_model: event.target.value, default_model: event.target.value })}
                    className="rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm outline-none focus:border-surface-500"
                  >
                    {(activeProfile?.models || []).map((model) => (
                      <option key={model} value={model}>{model}</option>
                    ))}
                  </select>
                </div>
              </div>

              <div className="mb-5 rounded-[1.5rem] border border-violet-200/70 bg-violet-50/55 p-4">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <div className="text-sm font-bold text-violet-950">当前能力路由</div>
                    <p className="mt-1 text-xs leading-5 text-violet-900/65">
                      按当前模型、默认模型和优先级预览实际候选顺序。真正执行时服务端仍会再次校验能力和可用状态。
                    </p>
                  </div>
                  <span className="shrink-0 rounded-full bg-white/80 px-2.5 py-1 text-[10px] font-semibold text-violet-700">
                    {activeProfile?.name || '当前服务'}
                  </span>
                </div>
                <div className="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-5">
                  {routePreview.map(({ capability, label, description, Icon, models }) => (
                    <div key={capability} className="min-w-0 rounded-2xl border border-violet-100 bg-white/80 p-3">
                      <div className="flex items-center gap-2 text-xs font-bold text-surface-800">
                        <Icon className="h-4 w-4 text-violet-600" />
                        {label}
                      </div>
                      <div className="mt-1 text-[10px] leading-4 text-surface-400">{description}</div>
                      {models.length > 0 ? (
                        <>
                          <div className="mt-2 truncate rounded-lg bg-surface-950 px-2 py-1.5 font-mono text-[10px] font-semibold text-white" title={models[0]}>
                            {models[0]}
                          </div>
                          {models.length > 1 && (
                            <div className="mt-1 truncate text-[10px] text-surface-400" title={models.slice(1).join(' → ')}>
                              备用：{models.slice(1).join(' → ')}
                            </div>
                          )}
                        </>
                      ) : (
                        <div className="mt-2 rounded-lg bg-amber-50 px-2 py-1.5 text-[10px] font-semibold text-amber-700">
                          未配置可用模型
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>

              <div className="grid gap-4 xl:grid-cols-2">
                {draft.llm_profiles.map((profile) => {
                  const isActive = draft.active_profile_id === profile.id
                  return (
                    <div key={profile.id} className={`rounded-[1.6rem] border p-4 shadow-sm transition-all ${isActive ? 'border-surface-900 bg-white ring-2 ring-surface-950/5' : 'border-black/[0.06] bg-white/75'}`}>
                      <div className="mb-4 flex items-start justify-between gap-3">
                        <div className="min-w-0 flex-1">
                          <input
                            value={profile.name}
                            onChange={(event) => updateProfile(profile.id, { name: event.target.value })}
                            className="w-full bg-transparent text-base font-bold text-surface-950 outline-none"
                            placeholder="配置名称"
                          />
                          <div className="mt-1 flex items-center gap-2 text-[11px] text-surface-400">
                            <KeyRound className="h-3 w-3" />
                            Key 池 {getProfileKeys(profile).length} 个
                          </div>
                        </div>
                        <div className="flex items-center gap-1.5">
                          {isActive ? (
                            <span className="inline-flex items-center gap-1 rounded-full bg-surface-950 px-2.5 py-1 text-[10px] font-bold text-white">
                              <Check className="h-3 w-3" />启用中
                            </span>
                          ) : (
                            <button onClick={() => setActiveProfile(profile.id)} className="rounded-full border border-black/10 bg-white px-3 py-1 text-xs font-semibold text-surface-600 hover:bg-surface-50">启用</button>
                          )}
                          <button
                            onClick={() => removeProfile(profile.id)}
                            disabled={draft.llm_profiles.length <= 1}
                            className="rounded-full p-2 text-surface-400 hover:bg-red-50 hover:text-red-600 disabled:opacity-30"
                            title="删除配置"
                          >
                            <Trash2 className="h-4 w-4" />
                          </button>
                        </div>
                      </div>

                      <div className="space-y-3">
                        <div className="grid gap-3 sm:grid-cols-3">
                          <label className="block text-xs font-semibold text-surface-500">
                            Provider
                            <input
                              value={profile.provider || ''}
                              onChange={(event) => updateProfile(profile.id, { provider: event.target.value })}
                              placeholder="OpenAI"
                              className="mt-1.5 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm text-surface-900 outline-none focus:border-surface-500"
                            />
                          </label>
                          <label className="block text-xs font-semibold text-surface-500">
                            Wire API
                            <select
                              value={profile.wire_api || 'chat_completions'}
                              onChange={(event) => updateProfile(profile.id, { wire_api: event.target.value })}
                              className="mt-1.5 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm text-surface-900 outline-none focus:border-surface-500"
                            >
                              <option value="chat_completions">Chat Completions</option>
                              <option value="responses">Responses</option>
                            </select>
                          </label>
                          <label className="block text-xs font-semibold text-surface-500">
                            Reasoning
                            <select
                              value={profile.reasoning_effort || 'medium'}
                              onChange={(event) => updateProfile(profile.id, { reasoning_effort: event.target.value })}
                              className="mt-1.5 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm text-surface-900 outline-none focus:border-surface-500"
                            >
                              <option value="none">None</option>
                              <option value="low">Low</option>
                              <option value="medium">Medium</option>
                              <option value="high">High</option>
                            </select>
                          </label>
                        </div>
                        <label className="block text-xs font-semibold text-surface-500">
                          Base URL
                          <input
                            value={profile.base_url}
                            onChange={(event) => updateProfile(profile.id, { base_url: event.target.value })}
                            className="mt-1.5 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm text-surface-900 outline-none focus:border-surface-500"
                          />
                          <button
                            type="button"
                            onClick={() => void fetchModels(profile)}
                            disabled={fetchingModelsId === profile.id || !profile.base_url.trim()}
                            className="mt-2 inline-flex items-center gap-1.5 rounded-full border border-black/10 bg-white px-3 py-1.5 text-[11px] font-semibold text-surface-600 hover:bg-surface-50 disabled:cursor-not-allowed disabled:opacity-50"
                          >
                            <RefreshCw className={`h-3.5 w-3.5 ${fetchingModelsId === profile.id ? 'animate-spin' : ''}`} />
                            {fetchingModelsId === profile.id ? '获取中...' : '从 Base URL 获取模型'}
                          </button>
                          {modelFetchResults[profile.id] && (
                            <div className={`mt-2 text-[11px] ${modelFetchResults[profile.id].ok ? 'text-emerald-600' : 'text-surface-500'}`}>
                              {modelFetchResults[profile.id].message}
                            </div>
                          )}
                        </label>
                        <div className="flex flex-wrap items-center gap-2">
                          <button
                            type="button"
                            onClick={() => void testLlm(profile)}
                            disabled={testingLlmId === profile.id || !profile.base_url.trim() || !(profile.default_model || profile.models[0])}
                            className="inline-flex items-center gap-1.5 rounded-full border border-black/10 bg-white px-3 py-1.5 text-[11px] font-semibold text-surface-600 hover:bg-surface-50 disabled:cursor-not-allowed disabled:opacity-50"
                          >
                            <Check className={`h-3.5 w-3.5 ${testingLlmId === profile.id ? 'animate-pulse' : ''}`} />
                            {testingLlmId === profile.id ? '测试中...' : '测试当前模型连接'}
                          </button>
                          {llmTestResults[profile.id] && (
                            <div className={`text-[11px] ${llmTestResults[profile.id].ok ? 'text-emerald-600' : 'text-red-600'}`}>
                              {llmTestResults[profile.id].message}
                            </div>
                          )}
                        </div>
                        <label className="block text-xs font-semibold text-surface-500">
                          <span className="flex items-center justify-between gap-3">
                            <span>API Key 池（每行一个，按请求轮询负载）</span>
                            <button
                              type="button"
                              onClick={() => setVisibleApiKeyProfiles((prev) => ({ ...prev, [profile.id]: !prev[profile.id] }))}
                              className="inline-flex shrink-0 items-center gap-1 rounded-full border border-black/10 bg-white px-2.5 py-1 text-[11px] font-semibold text-surface-600 hover:bg-surface-50"
                              aria-label={visibleApiKeyProfiles[profile.id] ? '隐藏 API Key' : '显示 API Key'}
                            >
                              {visibleApiKeyProfiles[profile.id] ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
                              {visibleApiKeyProfiles[profile.id] ? '隐藏' : '显示'}
                            </button>
                          </span>
                          <textarea
                            value={visibleApiKeyProfiles[profile.id] ? getProfileKeys(profile).join('\n') : maskApiKeys(getProfileKeys(profile))}
                            readOnly={!visibleApiKeyProfiles[profile.id]}
                            onChange={(event) => {
                              if (!visibleApiKeyProfiles[profile.id]) return
                              const apiKeys = event.target.value.split('\n').map((item) => item.trim()).filter(Boolean)
                              updateProfile(profile.id, { api_keys: apiKeys, api_key: '', has_api_key: apiKeys.length > 0 })
                            }}
                            rows={4}
                            placeholder={visibleApiKeyProfiles[profile.id] ? 'sk-...' : '已隐藏，点击“显示”后编辑'}
                            autoComplete="off"
                            spellCheck={false}
                            className={`mt-1.5 w-full resize-y rounded-2xl border border-black/10 px-3 py-2.5 font-mono text-xs text-surface-900 outline-none focus:border-surface-500 ${visibleApiKeyProfiles[profile.id] ? 'bg-white' : 'cursor-default bg-surface-50 text-surface-500'}`}
                          />
                        </label>
                        <label className="block text-xs font-semibold text-surface-500">
                          模型列表（逗号分隔）
                          <input
                            value={(profile.models || []).join(', ')}
                            onChange={(event) => {
                              const models = event.target.value.split(',').map((item) => item.trim()).filter(Boolean)
                              updateProfile(profile.id, { models, default_model: models[0] || profile.default_model })
                            }}
                            className="mt-1.5 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm text-surface-900 outline-none focus:border-surface-500"
                          />
                        </label>
                        <div className="rounded-2xl border border-black/[0.06] bg-[#f8f5ee]/70 p-3">
                          <div className="mb-2 flex items-center justify-between gap-3">
                            <div>
                              <div className="text-xs font-bold text-surface-700">模型能力</div>
                              <div className="mt-0.5 text-[11px] font-normal text-surface-400">勾选后，系统会按任务自动选择对应模型</div>
                            </div>
                            <span className="rounded-full bg-white px-2 py-1 text-[10px] font-semibold text-surface-400">未测试模型显示为待确认</span>
                          </div>
                          <div className="mb-2 rounded-xl border border-amber-200/70 bg-amber-50/80 px-2.5 py-2 text-[10px] leading-4 text-amber-700">
                            文本、视觉测试只发送最小请求；文生图、图生图和视频测试会调用真实媒体接口，可能产生上游费用。
                          </div>
                          <div className="space-y-2">
                            {getModelConfigs(profile).map((config) => (
                              <div key={config.model} className="rounded-xl border border-black/[0.06] bg-white/80 p-2.5">
                                <div className="flex items-center justify-between gap-2">
                                  <div className="min-w-0 truncate font-mono text-[11px] font-semibold text-surface-700" title={config.model}>{config.model}</div>
                                  <div className="flex shrink-0 items-center gap-2">
                                    <select
                                      value={config.priority}
                                      onChange={(event) => updateModelConfig(profile, config.model, { priority: Number(event.target.value) || 0 })}
                                      className="rounded-lg border border-black/10 bg-white px-1.5 py-1 text-[10px] text-surface-600 outline-none"
                                      aria-label={`${config.model} 优先级`}
                                    >
                                      {[0, 1, 2, 3].map((priority) => <option key={priority} value={priority}>优先级 {priority}</option>)}
                                    </select>
                                    <label className="inline-flex items-center gap-1 text-[10px] font-semibold text-surface-500">
                                      <input
                                        type="checkbox"
                                        checked={config.enabled}
                                        onChange={(event) => updateModelConfig(profile, config.model, { enabled: event.target.checked })}
                                      />
                                      启用
                                    </label>
                                  </div>
                                </div>
                                <div className="mt-2 flex flex-wrap gap-1.5">
                                  {capabilityOptions.map((capability) => {
                                    const selected = config.capabilities.includes(capability.value)
                                    return (
                                      <button
                                        key={capability.value}
                                        type="button"
                                        onClick={() => toggleModelCapability(profile, config.model, capability.value)}
                                        className={`rounded-full border px-2 py-1 text-[10px] font-semibold transition-colors ${selected ? 'border-surface-950 bg-surface-950 text-white' : 'border-black/10 bg-white text-surface-400 hover:border-surface-400 hover:text-surface-700'}`}
                                        aria-pressed={selected}
                                      >
                                        {capability.label}
                                      </button>
                                    )
                                  })}
                                  <span className={`rounded-full px-2 py-1 text-[10px] font-semibold ${config.status === 'available' ? 'bg-emerald-50 text-emerald-700' : config.status === 'unavailable' ? 'bg-red-50 text-red-700' : 'bg-amber-50 text-amber-700'}`}>
                                    {config.status === 'available' ? '可用' : config.status === 'unavailable' ? '不可用' : '待确认'}
                                  </span>
                                  {conversationCapabilities.some((capability) => config.capabilities.includes(capability)) && (
                                    <button
                                      type="button"
                                      onClick={() => void testModelCapabilities(profile, config, 'conversation')}
                                      disabled={testingCapabilityKey === `${profile.id}:${config.model}:conversation`}
                                      className="inline-flex items-center gap-1 rounded-full border border-violet-200 bg-violet-50 px-2 py-1 text-[10px] font-semibold text-violet-700 hover:bg-violet-100 disabled:cursor-not-allowed disabled:opacity-50"
                                      title="发送最小文本/工具/视觉请求，不调用媒体生成"
                                    >
                                      <FlaskConical className={`h-3 w-3 ${testingCapabilityKey === `${profile.id}:${config.model}:conversation` ? 'animate-pulse' : ''}`} />
                                      {testingCapabilityKey === `${profile.id}:${config.model}:conversation` ? '测试中' : '测试对话能力'}
                                    </button>
                                  )}
                                  {mediaCapabilities.some((capability) => config.capabilities.includes(capability)) && (
                                    <button
                                      type="button"
                                      onClick={() => void testModelCapabilities(profile, config, 'media')}
                                      disabled={testingCapabilityKey === `${profile.id}:${config.model}:media`}
                                      className="inline-flex items-center gap-1 rounded-full border border-orange-200 bg-orange-50 px-2 py-1 text-[10px] font-semibold text-orange-700 hover:bg-orange-100 disabled:cursor-not-allowed disabled:opacity-50"
                                      title="调用真实图片/视频生成接口，可能产生上游费用"
                                    >
                                      <FlaskConical className={`h-3 w-3 ${testingCapabilityKey === `${profile.id}:${config.model}:media` ? 'animate-pulse' : ''}`} />
                                      {testingCapabilityKey === `${profile.id}:${config.model}:media` ? '测试中' : '测试媒体能力'}
                                    </button>
                                  )}
                                </div>
                                {capabilityOptions.some((capability) => config.capability_statuses?.[capability.value]) && (
                                  <div className="mt-2 flex flex-wrap gap-1">
                                    {capabilityOptions.filter((capability) => config.capabilities.includes(capability.value) && config.capability_statuses?.[capability.value]).map((capability) => {
                                      const status = config.capability_statuses?.[capability.value]
                                      return (
                                        <span key={`${config.model}-${capability.value}-status`} className={`rounded-full px-2 py-0.5 text-[9px] font-semibold ${status === 'available' ? 'bg-emerald-50 text-emerald-700' : status === 'unavailable' ? 'bg-red-50 text-red-700' : 'bg-amber-50 text-amber-700'}`}>
                                          {capability.label}：{status === 'available' ? '可用' : status === 'unavailable' ? '不可用' : '待确认'}
                                        </span>
                                      )
                                    })}
                                  </div>
                                )}
                                {(['conversation', 'media'] as const).map((scope) => {
                                  const result = capabilityTestResults[`${profile.id}:${config.model}:${scope}`]
                                  if (!result) return null
                                  return (
                                    <div key={`${config.model}-${scope}-result`} className={`mt-2 rounded-lg px-2 py-1.5 text-[10px] leading-4 ${result.ok ? 'bg-emerald-50 text-emerald-700' : 'bg-red-50 text-red-600'}`}>
                                      <div>{result.message}</div>
                                      {result.results && result.results.length > 1 && (
                                        <div className="mt-1 space-y-0.5 text-[9px] opacity-90">
                                          {result.results.map((item) => <div key={`${item.capability}-${item.message}`}>{capabilityOptions.find((option) => option.value === item.capability)?.label || item.capability}：{item.ok ? '通过' : item.message}</div>)}
                                        </div>
                                      )}
                                    </div>
                                  )
                                })}
                              </div>
                            ))}
                            {getModelConfigs(profile).length === 0 && (
                              <div className="py-2 text-center text-[11px] text-surface-400">请先填写模型名或从 Base URL 获取模型</div>
                            )}
                          </div>
                        </div>
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          )}

          {section === 'base' && (
            <div>
              <h2 className="text-xl font-bold tracking-tight text-surface-950">基础信息</h2>
              <p className="mt-1 text-sm text-surface-500">配置产品名称、工作区标题和默认主题。</p>
              <div className="mt-5 grid gap-4 md:grid-cols-2">
                <label className="block text-sm font-semibold text-surface-600">
                  产品名称
                  <input
                    value={draft.basic.app_name}
                    onChange={(event) => updateDraft({ basic: { ...draft.basic, app_name: event.target.value } })}
                    className="mt-2 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm outline-none focus:border-surface-500"
                  />
                </label>
                <label className="block text-sm font-semibold text-surface-600">
                  工作区标题
                  <input
                    value={draft.basic.workspace_title}
                    onChange={(event) => updateDraft({ basic: { ...draft.basic, workspace_title: event.target.value } })}
                    className="mt-2 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm outline-none focus:border-surface-500"
                  />
                </label>
                <label className="block text-sm font-semibold text-surface-600 md:col-span-2">
                  品牌副标题
                  <input
                    value={draft.basic.brand_tagline}
                    onChange={(event) => updateDraft({ basic: { ...draft.basic, brand_tagline: event.target.value } })}
                    className="mt-2 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm outline-none focus:border-surface-500"
                  />
                </label>
                <label className="block text-sm font-semibold text-surface-600">
                  默认主题
                  <select
                    value={draft.basic.default_theme}
                    onChange={(event) => updateDraft({ basic: { ...draft.basic, default_theme: event.target.value } })}
                    className="mt-2 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm outline-none focus:border-surface-500"
                  >
                    {['default', 'business', 'tech', 'warm', 'minimal'].map((theme) => (
                      <option key={theme} value={theme}>{theme}</option>
                    ))}
                  </select>
                </label>
              </div>
            </div>
          )}

          {section === 'mcp' && (
            <div>
              <div className="mb-5 flex items-start justify-between gap-4">
                <div>
                  <h2 className="text-xl font-bold tracking-tight text-surface-950">MCP 服务</h2>
                  <p className="mt-1 text-sm text-surface-500">保存常用 MCP 服务的地址和启用状态，方便后续接入。</p>
                </div>
                <button onClick={addMcp} className="inline-flex items-center gap-1.5 rounded-full bg-surface-950 px-4 py-2 text-sm font-semibold text-white hover:bg-surface-800">
                  <Plus className="h-4 w-4" />
                  添加服务
                </button>
              </div>

              <div className="space-y-4">
                {draft.mcp_servers.length === 0 && (
                  <div className="rounded-[1.6rem] border border-dashed border-black/10 bg-white/65 px-4 py-8 text-center text-sm text-surface-500">
                    还没有 MCP 服务配置，添加后会保存在当前账号下。
                  </div>
                )}
                {draft.mcp_servers.map((server) => (
                  <div key={server.id} className="rounded-[1.6rem] border border-black/[0.06] bg-white/75 p-4 shadow-sm">
                    <div className="mb-4 flex items-start justify-between gap-3">
                      <div className="min-w-0 flex-1">
                        <input
                          value={server.name}
                          onChange={(event) => updateMcp(server.id, { name: event.target.value })}
                          className="w-full bg-transparent text-base font-bold text-surface-950 outline-none"
                          placeholder="服务名称"
                        />
                        <div className="mt-1 text-[11px] text-surface-400">{server.enabled ? '已启用' : '已停用'}</div>
                      </div>
                      <div className="flex items-center gap-2">
                        <button
                          type="button"
                          onClick={() => testMcp(server)}
                          disabled={testingMcpId === server.id}
                          className="rounded-full border border-black/10 bg-white px-3 py-1 text-xs font-semibold text-surface-600 hover:bg-surface-50 disabled:cursor-not-allowed disabled:opacity-50"
                        >
                          {testingMcpId === server.id ? '测试中...' : '测试连接'}
                        </button>
                        <label className="flex items-center gap-2 text-xs font-semibold text-surface-500">
                          <input
                            type="checkbox"
                            checked={server.enabled}
                            onChange={(event) => updateMcp(server.id, { enabled: event.target.checked })}
                          />
                          启用
                        </label>
                        <button onClick={() => removeMcp(server.id)} className="rounded-full p-2 text-surface-400 hover:bg-red-50 hover:text-red-600" title="删除服务">
                          <Trash2 className="h-4 w-4" />
                        </button>
                      </div>
                    </div>

                    <div className="grid gap-3 md:grid-cols-2">
                      <label className="block text-xs font-semibold text-surface-500">
                        传输方式
                        <select
                          value={server.transport}
                          onChange={(event) => updateMcp(server.id, { transport: event.target.value })}
                          className="mt-1.5 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm outline-none focus:border-surface-500"
                        >
                          <option value="http">HTTP</option>
                          <option value="sse">SSE</option>
                          <option value="stdio">STDIO</option>
                        </select>
                      </label>
                      <label className="block text-xs font-semibold text-surface-500">
                        服务地址
                        <input
                          value={server.endpoint}
                          onChange={(event) => updateMcp(server.id, { endpoint: event.target.value })}
                          className="mt-1.5 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm outline-none focus:border-surface-500"
                        />
                      </label>
                      <label className="block text-xs font-semibold text-surface-500 md:col-span-2">
                        描述
                        <input
                          value={server.description || ''}
                          onChange={(event) => updateMcp(server.id, { description: event.target.value })}
                          className="mt-1.5 w-full rounded-2xl border border-black/10 bg-white px-3 py-2.5 text-sm outline-none focus:border-surface-500"
                        />
                      </label>
                    </div>

                    {mcpTestResults[server.id] && (
                      <div className={`mt-3 rounded-2xl border px-3 py-2 text-xs ${mcpTestResults[server.id].ok ? 'border-emerald-200 bg-emerald-50 text-emerald-700' : 'border-red-200 bg-red-50 text-red-600'}`}>
                        <div className="font-semibold">{mcpTestResults[server.id].message}</div>
                        {mcpTestResults[server.id].tools.length > 0 && (
                          <div className="mt-1 text-[11px]">
                            可用工具：{mcpTestResults[server.id].tools.join(' / ')}
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="mt-6 flex items-center justify-end gap-3 border-t border-black/[0.06] pt-5">
            <button onClick={onClose} className="rounded-full border border-black/10 bg-white px-4 py-2 text-sm font-semibold text-surface-600 hover:bg-surface-50">
              取消
            </button>
            <button
              onClick={handleSave}
              disabled={!hasChanges || saving}
              className="rounded-full bg-surface-950 px-5 py-2 text-sm font-semibold text-white disabled:cursor-not-allowed disabled:opacity-40"
            >
              {saving ? '保存中...' : '保存设置'}
            </button>
          </div>
        </main>
      </div>
    </div>
  )
}
