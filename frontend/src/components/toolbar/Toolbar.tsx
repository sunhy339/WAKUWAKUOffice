import { ChevronLeft, ChevronRight, ZoomIn, ZoomOut, Maximize2 } from 'lucide-react'
import { useState } from 'react'
import { usePPTStore } from '@/stores/ppt-store'

export function Toolbar({ compact = false }: { compact?: boolean }) {
  const { slides, currentSlideIndex, setCurrentSlide } = usePPTStore()
  const [zoom, setZoom] = useState(1)

  const canPrev = currentSlideIndex > 0
  const canNext = currentSlideIndex < slides.length - 1

  return (
    <div className={`h-10 shrink-0 border-b border-surface-200 bg-white flex items-center justify-between ${compact ? 'px-2' : 'px-4'}`}>
      {/* 左侧：翻页 */}
      <div className="flex items-center gap-1">
        <button
          onClick={() => canPrev && setCurrentSlide(currentSlideIndex - 1)}
          disabled={!canPrev}
          className="btn-ghost h-8 w-8 p-0"
          title="上一页"
        >
          <ChevronLeft className="w-4 h-4" />
        </button>
        <span className={`text-surface-500 ${compact ? 'px-1 text-xs' : 'px-2 text-sm'}`}>
          {slides.length > 0 ? `${currentSlideIndex + 1} / ${slides.length}` : '0 / 0'}
        </span>
        <button
          onClick={() => canNext && setCurrentSlide(currentSlideIndex + 1)}
          disabled={!canNext}
          className="btn-ghost h-8 w-8 p-0"
          title="下一页"
        >
          <ChevronRight className="w-4 h-4" />
        </button>
      </div>

      {/* 右侧：缩放 */}
      <div className={`flex items-center gap-1 ${compact ? 'shrink-0' : ''}`}>
        <button
          onClick={() => setZoom(Math.max(0.5, zoom - 0.1))}
          className="btn-ghost h-8 w-8 p-0"
          title="缩小"
        >
          <ZoomOut className="w-4 h-4" />
        </button>
        <span className="w-11 text-center text-[11px] text-surface-400">
          {Math.round(zoom * 100)}%
        </span>
        <button
          onClick={() => setZoom(Math.min(2, zoom + 0.1))}
          className="btn-ghost h-8 w-8 p-0"
          title="放大"
        >
          <ZoomIn className="w-4 h-4" />
        </button>
        <div className="w-px h-5 bg-surface-200 mx-1" />
        <button
          onClick={() => setZoom(1)}
          className="btn-ghost h-8 px-2 text-xs"
          title="重置"
        >
          <Maximize2 className="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  )
}
