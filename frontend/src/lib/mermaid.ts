/**
 * 从模型回复中提取 Mermaid 源码。
 *
 * 模型服务偶尔会在 draw.io XML 不可用时返回 Mermaid 代码。提取逻辑放在
 * 独立适配层，供历史兼容恢复和预览层共同使用，避免把 Markdown 文本解析
 * 规则散落在页面组件中。
 */
export function extractMermaidSource(text: string): string | null {
  const value = text.trim()
  if (!value) return null

  const fencedMatches = value.matchAll(/```(?:mermaid|less|markdown|md)?\s*\n?([\s\S]*?)```/gi)
  for (const match of fencedMatches) {
    const source = normalizeMermaidSource(match[1])
    if (source) return source
  }

  return normalizeMermaidSource(value)
}

function normalizeMermaidSource(value: string): string | null {
  const lines = value.trim().split(/\r?\n/)
  const startIndex = lines.findIndex((line) => /^\s*(flowchart|graph)\s+(TD|TB|BT|RL|LR)\b/i.test(line))
  if (startIndex < 0) return null

  const source = lines
    .slice(startIndex)
    .join('\n')
    .replace(/\s*```\s*$/g, '')
    .trim()
  return /^(flowchart|graph)\s+(TD|TB|BT|RL|LR)\b/i.test(source) ? source : null
}
