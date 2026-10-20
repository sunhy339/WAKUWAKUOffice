import type { ChatSSEEvent, ChatSSEEventDataMap, ChatSSEEventName } from '@/types'

export const CHAT_SSE_EVENT_NAMES: readonly ChatSSEEventName[] = [
  'message',
  'slide_update',
  'project_update',
  'artifact_update',
  'tool_result',
  'state_update',
  'done',
  'error',
]

const CHAT_SSE_EVENT_NAME_SET = new Set<string>(CHAT_SSE_EVENT_NAMES)

export function parseSseData(data: string): unknown {
  try {
    return JSON.parse(data)
  } catch {
    // SSE 允许 data 是普通文本；保留原始值比丢弃事件更安全。
    return data
  }
}

export function normalizeChatSseEvent(
  event: string,
  data: unknown,
  sequence: number,
): ChatSSEEvent {
  const normalizedEvent = event.trim() || 'message'
  return {
    event: normalizedEvent,
    data: data as ChatSSEEventDataMap[string],
    sequence,
    received_at: new Date().toISOString(),
    known: CHAT_SSE_EVENT_NAME_SET.has(normalizedEvent),
  }
}

export function isTerminalChatSseEvent(event: ChatSSEEvent): boolean {
  return event.event === 'done' || event.event === 'error'
}

/**
 * 用事件名和负载生成本次连接内的去重键。
 *
 * 服务端重试或代理重放 SSE 时，可能重复发送同一条业务事件；保留
 * 事件顺序字段用于观测，但不要让重复事件再次驱动页面状态或日志。
 */
export function getChatSseEventKey(event: ChatSSEEvent): string {
  let payload = ''
  if (typeof event.data === 'string') {
    payload = event.data
  } else {
    try {
      payload = JSON.stringify(event.data)
    } catch {
      payload = String(event.data)
    }
  }
  return `${event.event}:${payload}`
}

