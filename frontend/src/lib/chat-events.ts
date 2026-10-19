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

