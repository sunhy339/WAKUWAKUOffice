// ===== 工具智能体 =====
export type ToolKind = 'general' | 'ppt' | 'doc' | 'drawio' | 'excel' | 'image' | 'video' | 'code';

export type ArtifactKind = 'document' | 'markdown' | 'ppt' | 'drawio' | 'sheet' | 'chart' | 'image' | 'video' | 'code' | 'mixed' | 'search';

export interface ChatAttachment {
  id: string;
  name: string;
  kind: 'text' | 'image';
  mime_type: string;
  size: number;
  text_content?: string;
  data_url?: string;
  original_size?: number;
  width?: number;
  height?: number;
  compressed?: boolean;
  from_artifact?: boolean;
}

/** @ 引用产物的标签（输入框内显示为 chip） */
export interface InputRef {
  id: string;
  artifactId: string;
  kind: ArtifactKind;
  title: string;
  /** 发送时拼接的引用文本 */
  refText: string;
  /** 来源会话 ID（当前会话或历史会话） */
  sessionId?: string;
  /** 产物内容摘要（发送时传给后端，让 AI 识别引用内容） */
  contentSummary?: string;
}

export interface LLMProfile {
  id: string;
  name: string;
  base_url: string;
  api_keys?: string[];
  models: string[];
  default_model: string;
  api_key?: string;
  has_api_key?: boolean;
}

export interface BasicSettings {
  app_name: string;
  workspace_title: string;
  brand_tagline: string;
  default_theme: string;
}

export interface MCPServiceConfig {
  id: string;
  name: string;
  transport: string;
  endpoint: string;
  enabled: boolean;
  description?: string;
}

export interface AppSettings {
  llm_profiles: LLMProfile[];
  active_profile_id: string;
  default_model: string;
  active_model: string;
  basic: BasicSettings;
  mcp_servers: MCPServiceConfig[];
  updated_at: string;
}

export interface Artifact {
  id: string;
  kind: ArtifactKind;
  tool_kind: ToolKind;
  title: string;
  status: 'draft' | 'generating' | 'ready' | 'error';
  content: any;
  version: number;
  created_at: string;
  updated_at: string;
}

export interface ToolConfigOption {
  key: string;
  label: string;
  type: 'select' | 'toggle';
  options?: Array<{ value: string; label: string; description?: string }>;
  defaultValue: string | boolean;
}

export interface AgentToolConfig {
  id: ToolKind;
  name: string;
  shortName: string;
  description: string;
  artifactLabel: string;
  promptPlaceholder: string;
  examples: string[];
  configOptions?: ToolConfigOption[];
}

export type ToolConfigMap = Partial<Record<string, string | boolean>>;

export interface ConversationRecord {
  id: string;
  title: string;
  tool: ToolKind;
  summary?: string;
  updated_at: string;
  message_count: number;
  order_col?: number;
  project_id?: string;
  project_title?: string;
}

export interface ProjectMeta {
  id: string;
  title: string;
  description?: string;
  tool_kind?: ToolKind;
  session_count: number;
  sessions?: ConversationRecord[];
  created_at: string;
  updated_at: string;
}

export interface PersistedSession {
  id: string;
  owner_id: string;
  messages: Array<{
    role: 'user' | 'assistant' | 'system' | 'tool';
    content: string;
    tool_calls?: Array<{ function?: { name?: string; arguments?: string } }>;
    tool_call_id?: string;
    created_at?: string;
  }>;
  artifacts?: Artifact[];
  project_id?: string;
  tool_kind?: ToolKind;
  title: string;
  summary?: string;
  order_col?: number;
  created_at: string;
  updated_at: string;
}

/** 共享类型定义 - aippt.xiaofuge.cn */

// ===== 幻灯片元素 =====
export interface SlideElement {
  type: 'text' | 'image' | 'shape' | 'table' | 'chart';
  x: number;
  y: number;
  w: number;
  h: number;
  text?: string;
  fontSize?: number;
  color?: string;
  bold?: boolean;
  italic?: boolean;
  align?: 'left' | 'center' | 'right';
  valign?: 'top' | 'middle' | 'bottom';
  fill?: string;
  path?: string;
  shape?: string;
  rows?: number;
  cols?: number;
  table_data?: string[][];
  chart_type?: string;
  chart_data?: any;
}

// ===== 幻灯片 =====
export interface Slide {
  id: string;
  index: number;
  layout: 'title' | 'content' | 'two-column' | 'image' | 'chart' | 'section';
  background: string;
  elements: SlideElement[];
  notes?: string;
  title?: string;
}

// ===== 绘制历史 =====
export interface ProjectHistoryEntry {
  id: string;
  type: 'create' | 'plan' | 'draw' | 'layout' | 'export' | 'system';
  title: string;
  detail?: string;
  slide_index?: number;
  slide_title?: string;
  created_at: string;
}

// ===== PPT 项目 =====
export interface PPTProject {
  id: string;
  title: string;
  theme: string;
  slides: Slide[];
  history?: ProjectHistoryEntry[];
  layout: '16x9' | '4x3';
  created_at: string;
  updated_at: string;
  owner_id?: string;
}

// ===== 对话消息 =====
export interface ChatMessage {
  role: 'user' | 'assistant';
  content: string;
  timestamp: string;
  action?: string;
  slide_id?: string;
  attachments?: ChatAttachment[];
  /** 用户消息引用的产物标签 */
  inputRefs?: InputRef[];
}

// ===== 用户 =====
export interface User {
  id: string;
  username: string;
  email?: string;
  avatar?: string;
  role?: string;
}

export interface TokenResponse {
  access_token: string;
  token_type: string;
  user: User;
}

// ===== Agent Trace =====
export type AgentTraceKind = 'state' | 'tool' | 'artifact' | 'error';

export interface AgentTraceEvent {
  id: string;
  kind: AgentTraceKind;
  title: string;
  detail?: string;
  status: 'running' | 'success' | 'error';
  tool?: string;
  at: string;
}

// ===== SSE 事件 =====
export type ChatSSEEventName =
  | 'message'
  | 'slide_update'
  | 'project_update'
  | 'artifact_update'
  | 'tool_result'
  | 'state_update'
  | 'done'
  | 'error';

export type ChatStreamPhase = 'idle' | 'thinking' | 'generating' | 'finishing' | 'done' | 'error';

export interface ChatMessageEventData {
  text?: string;
  session_id?: string;
  project_id?: string;
  start?: boolean;
}

export interface ChatStateUpdateEventData {
  phase?: string;
  step?: string;
  detail?: string;
  at?: string;
  attachment_count?: number;
}

export interface ChatToolResultEventData {
  tool?: string;
  success?: boolean;
  result?: Record<string, any>;
  error?: string | null;
}

export interface ChatArtifactUpdateEventData {
  artifact?: Artifact;
  artifacts?: Artifact[];
  session_id?: string;
  tool_kind?: ToolKind;
}

export interface ChatProjectUpdateEventData {
  project_id?: string;
  title?: string;
  theme?: string;
  slides?: Slide[];
  slide_count?: number;
  total_slides?: number;
  history?: ProjectHistoryEntry[];
  current_index?: number;
}

export type ChatSlideUpdateEventData = ChatProjectUpdateEventData;

export interface ChatDoneEventData {
  session_id?: string;
  summary?: string;
  artifacts?: Artifact[];
  new_artifacts?: Artifact[];
  project_id?: string;
}

export interface ChatErrorEventData {
  message?: string;
  detail?: string;
}

/**
 * SSE 事件负载映射。
 * 保留字符串索引签名，允许服务端未来增加事件而不破坏旧版前端。
 */
export interface ChatSSEEventDataMap {
  [event: string]: any;
  message: ChatMessageEventData;
  slide_update: ChatSlideUpdateEventData;
  project_update: ChatProjectUpdateEventData;
  artifact_update: ChatArtifactUpdateEventData;
  tool_result: ChatToolResultEventData;
  state_update: ChatStateUpdateEventData;
  done: ChatDoneEventData;
  error: ChatErrorEventData;
}

export interface ChatSSEEvent<TEvent extends string = string> {
  /** 服务端事件名；未知事件会被保留，方便兼容后端增量发布。 */
  event: TEvent;
  /** 已解析的 JSON 负载；非 JSON 数据以字符串保留。 */
  data: ChatSSEEventDataMap[TEvent];
  /** 本次连接内由前端分配的单调递增序号。 */
  sequence: number;
  /** 前端收到事件的时间，不代表服务端产生时间。 */
  received_at: string;
  /** 是否属于当前已知事件集合。 */
  known: boolean;
}

// ===== 文件管理 =====
export interface FileItem {
  id: string;
  name: string;
  file_type: string;
  file_size: number;
  folder_id?: string;
  description?: string;
  metadata?: Record<string, any>;
  created_at: string;
  updated_at: string;
}

export interface Folder {
  id: string;
  owner_id: string;
  name: string;
  parent_id?: string;
  created_at: string;
  updated_at: string;
}

export interface FileStats {
  by_type: Record<string, number>;
  total_size: number;
  total_files: number;
}

// ===== 通知 =====
export interface Notification {
  id: string;
  type: 'system' | 'project' | 'message';
  title: string;
  content?: string;
  is_read: boolean;
  link?: string;
  created_at: string;
}

