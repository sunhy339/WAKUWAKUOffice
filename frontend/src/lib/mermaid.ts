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

/**
 * 为 Mermaid 预览准备可解析的源码，但不改写用户看到或下载的原始源码。
 *
 * Mermaid flowchart 的 class 名称也会经过语法解析，`end` 是保留字。模型
 * 很容易生成 `classDef end ...`，导致整张图解析失败，因此只在渲染前给
 * 这类样式名加上内部别名。
 */
export function prepareMermaidSourceForRender(source: string): string {
  const aliases = new Map<string, string>()
  const lines = source.split(/\r?\n/).map((line) => {
    const match = line.match(/^(\s*classDef\s+)([A-Za-z_][\w-]*)(\b.*)$/)
    if (!match || match[2].toLowerCase() !== 'end') return line
    const alias = `${match[2]}Node`
    aliases.set(match[2], alias)
    return `${match[1]}${alias}${match[3]}`
  })

  if (aliases.size === 0) return source

  return lines.map((line) => {
    if (!/^\s*class\s+/.test(line)) return line
    let next = line
    for (const [original, alias] of aliases) {
      next = next.replace(new RegExp(`\\b${original}\\b(?=\\s*;?\\s*$)`, 'i'), alias)
    }
    return next
  }).join('\n')
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
