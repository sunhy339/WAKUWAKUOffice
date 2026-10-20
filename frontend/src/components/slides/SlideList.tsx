import { FileText } from 'lucide-react'
import { useEffect, useRef } from 'react'
import type { Slide } from '@/types'
import { SlideThumbnail } from '@/components/slides/SlideThumbnail'

interface SlideListProps {
  slides: Slide[]
  currentIndex: number
  onSelect: (index: number) => void
  orientation?: 'vertical' | 'horizontal'
  compact?: boolean
}

export function SlideList({ slides, currentIndex, onSelect, orientation = 'vertical', compact = false }: SlideListProps) {
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([])
  const isHorizontal = orientation === 'horizontal'

  useEffect(() => {
    itemRefs.current[currentIndex]?.scrollIntoView({
      block: 'nearest',
      behavior: 'smooth',
    })
  }, [currentIndex])

  return (
    <div className={isHorizontal ? 'px-3 py-2' : 'p-3'}>
      <div className={`flex items-center justify-between ${isHorizontal ? 'mb-2' : 'mb-3'}`}>
        <span className="text-xs font-medium text-surface-500 uppercase tracking-wide">
          幻灯片 ({slides.length})
        </span>
      </div>

      <div className={isHorizontal ? 'flex gap-2 overflow-x-auto pb-1' : 'space-y-2'}>
        {slides.map((slide, i) => (
          <button
            key={slide.id}
            ref={(node) => {
              itemRefs.current[i] = node
            }}
            onClick={() => onSelect(i)}
            aria-label={`选择第 ${i + 1} 页${slide.title ? `：${slide.title}` : ''}`}
            className={`${isHorizontal ? 'w-[88px] shrink-0' : 'w-full'} group relative overflow-hidden rounded-lg border-2 transition-all ${
              i === currentIndex
                ? 'border-primary-500 ring-2 ring-primary-100'
                : 'border-surface-200 hover:border-surface-300'
            }`}
          >
            {/* 页码 */}
            <div className="absolute top-1 left-1 z-10 w-5 h-5 bg-black/50 text-white text-xs rounded flex items-center justify-center">
              {i + 1}
            </div>

            {/* 缩略图 */}
            <div className="aspect-video bg-white">
              <SlideThumbnail slide={slide} />
            </div>

            {/* 标题 */}
            {slide.title && !compact && (
              <div className="px-2 py-1 text-xs text-surface-600 truncate bg-surface-50">
                {slide.title}
              </div>
            )}
            {compact && <div className="bg-surface-50 px-1 py-1 text-center text-[10px] text-surface-500">第 {i + 1} 页</div>}
          </button>
        ))}

        {/* 空状态提示 */}
        {slides.length === 0 && (
          <div className="text-center py-8 text-surface-300">
            <FileText className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p className="text-xs">暂无幻灯片</p>
          </div>
        )}
      </div>
    </div>
  )
}
