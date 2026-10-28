import { Box, Camera, Sparkles } from 'lucide-react'
import { getStyle, type StyleCategory } from '@shared/styles'
import { cx } from './ui'

/**
 * Sample images for each visual style, generated with GPT Image 2.5. Each shows a different front-facing
 * character in that style, centered, so the pictures also work cropped to a square.
 */
const THUMBS = import.meta.glob('../assets/styles/*.webp', { eager: true, import: 'default' }) as Record<string, string>

export function styleThumb(id: string): string | null {
  return THUMBS[`../assets/styles/${id}.webp`] ?? null
}

const CATEGORY_ICON: Record<StyleCategory, typeof Box> = { '2d': Sparkles, '3d': Box, realistis: Camera }

/** The style's sample image, or a colored tile when a style has no sample yet. */
export function StyleSwatch({ styleId, className, iconSize = 26 }: { styleId: string; className?: string; iconSize?: number }) {
  const thumb = styleThumb(styleId)
  if (thumb) {
    return (
      <span className={cx('block overflow-hidden', className)}>
        <img src={thumb} alt="" draggable={false} className="size-full object-cover" />
      </span>
    )
  }
  const s = getStyle(styleId)
  const Icon = CATEGORY_ICON[s.category]
  return (
    <span className={cx('flex items-center justify-center', className)} style={{ background: s.swatch, color: s.ink }}>
      <Icon style={{ width: iconSize, height: iconSize }} strokeWidth={1.6} />
    </span>
  )
}
