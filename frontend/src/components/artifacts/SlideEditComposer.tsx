import { useEffect, useState } from 'react'
import { Check, Loader2, Sparkles, X } from 'lucide-react'
import type { Slide } from '@/types'

interface SlideEditComposerProps {
  slide: Slide
  slideNumber: number
  isSubmitting: boolean
  onSubmit: (instruction: string) => void
  onClose: () => void
}
const QUICK_ACTIONS = [
  '精简文字，保留核心观点',
  '调整版式，让信息层级更清晰',
  '补充一段适合演示时讲解的说明',
  '把标题改得更具体、更有结论性',
]

export function SlideEditComposer({ slide, slideNumber, isSubmitting, onSubmit, onClose }: SlideEditComposerProps) {
  const [instruction, setInstruction] = useState('')

  useEffect(() => {
    setInstruction('')
  }, [slide.id])

  const submit = () => {
    const value = instruction.trim()
    if (!value || isSubmitting) return
    onSubmit(value)
  }

  return (
    <section className="shrink-0 border-b border-surface-200 bg-white/95 px-4 py-3 shadow-[0_8px_24px_rgba(24,24,27,0.06)]">
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-start gap-2">
          <div className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-xl bg-primary-50 text-primary-600">
            <Sparkles className="h-4 w-4" />
          </div>
          <div className="min-w-0">
            <div className="text-sm font-semibold text-surface-800">AI 修改第 {slideNumber} 页</div>
            <div className="mt-0.5 truncate text-[11px] text-surface-400">只修改当前页面，其他页面保持不变</div>
          </div>
        </div>
        <button
          type="button"
          className="btn-ghost h-7 w-7 shrink-0 px-0"
          onClick={onClose}
          disabled={isSubmitting}
          aria-label="关闭 AI 修改面板"
          title="关闭"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      <div className="mt-3 flex flex-wrap gap-1.5">
        {QUICK_ACTIONS.map((action) => (
          <button
            key={action}
            type="button"
            className="rounded-full border border-surface-200 bg-surface-50 px-2.5 py-1.5 text-[11px] text-surface-600 transition hover:border-primary-200 hover:bg-primary-50 hover:text-primary-700 disabled:cursor-not-allowed disabled:opacity-50"
            onClick={() => setInstruction(action)}
            disabled={isSubmitting}
          >
            {action}
          </button>
        ))}
      </div>

      <textarea
        value={instruction}
        onChange={(event) => setInstruction(event.target.value)}
        onKeyDown={(event) => {
          if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
            event.preventDefault()
            submit()
          }
        }}
        disabled={isSubmitting}
        rows={3}
        maxLength={4000}
        placeholder="例如：把这页的三段文字改成三个简短要点，并突出最后的结论"
        className="input mt-3 min-h-[76px] resize-y text-sm leading-6"
        aria-label={`第 ${slideNumber} 页修改要求`}
      />

      <div className="mt-2 flex items-center justify-between gap-3">
        <span className="text-[11px] text-surface-400">支持 Ctrl/⌘ + Enter 提交</span>
        <button
          type="button"
          className="btn-primary h-9 px-3 text-xs"
          onClick={submit}
          disabled={!instruction.trim() || isSubmitting}
        >
          {isSubmitting ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}
          {isSubmitting ? '修改中…' : '开始修改'}
        </button>
      </div>
    </section>
  )
}