import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode, type RefObject } from 'react'
import { createPortal } from 'react-dom'
import { cx } from './cx'

export type CloseReason = 'escape' | 'outside' | 'scroll' | 'resize'

/**
 * A floating panel anchored to a trigger. It is drawn in a portal with fixed positioning, so scrolling
 * panels never clip it. It opens toward the side with more room and closes on outside click, Escape,
 * window resize, or when the page behind it scrolls.
 */
export function Popover({
  anchor,
  open,
  onClose,
  width,
  align = 'left',
  maxHeight = 400,
  label,
  className,
  children
}: {
  anchor: RefObject<HTMLElement | null>
  open: boolean
  /** Escape should hand focus back to the trigger; the other reasons should not. */
  onClose: (reason: CloseReason) => void
  /** Width in pixels. Never narrower than the trigger. */
  width?: number
  /** Which edge of the trigger a wider panel lines up with. */
  align?: 'left' | 'right'
  maxHeight?: number
  label?: string
  className?: string
  children: ReactNode
}) {
  const panel = useRef<HTMLDivElement>(null)
  const close = useRef(onClose)
  close.current = onClose
  const [style, setStyle] = useState<CSSProperties | null>(null)

  useLayoutEffect(() => {
    const el = anchor.current
    if (!open || !el) {
      setStyle(null)
      return
    }
    const r = el.getBoundingClientRect()
    const vw = window.innerWidth
    const vh = window.innerHeight
    const gap = 6
    const margin = 12
    const w = Math.min(Math.max(width ?? r.width, r.width), vw - margin * 2)
    const below = vh - r.bottom - gap - margin
    const above = r.top - gap - margin
    const up = below < Math.min(maxHeight, 280) && above > below
    const left = Math.min(Math.max(align === 'right' ? r.right - w : r.left, margin), vw - w - margin)
    setStyle({
      ...(up ? { bottom: vh - r.top + gap } : { top: r.bottom + gap }),
      left,
      width: w,
      maxHeight: Math.max(160, Math.min(maxHeight, up ? above : below))
    })
  }, [open, width, align, maxHeight, anchor])

  useEffect(() => {
    if (!open) return
    const inside = (t: EventTarget | null): boolean => !!t && (!!panel.current?.contains(t as Node) || !!anchor.current?.contains(t as Node))
    const onDown = (e: MouseEvent): void => {
      if (!inside(e.target)) close.current('outside')
    }
    const onKey = (e: KeyboardEvent): void => {
      if (e.key !== 'Escape') return
      e.stopPropagation()
      close.current('escape')
    }
    const onScroll = (e: Event): void => {
      if (!panel.current?.contains(e.target as Node)) close.current('scroll')
    }
    const onResize = (): void => close.current('resize')
    window.addEventListener('mousedown', onDown)
    window.addEventListener('keydown', onKey, true)
    window.addEventListener('scroll', onScroll, true)
    window.addEventListener('resize', onResize)
    return () => {
      window.removeEventListener('mousedown', onDown)
      window.removeEventListener('keydown', onKey, true)
      window.removeEventListener('scroll', onScroll, true)
      window.removeEventListener('resize', onResize)
    }
  }, [open, anchor])

  if (!open || !style) return null
  return createPortal(
    <div
      ref={panel}
      aria-label={label}
      style={style}
      // React sends events from a portal up its component tree, so a click here would also reach
      // whatever the trigger sits in (e.g. the timeline would seek). Keep them inside the panel.
      onPointerDown={(e) => e.stopPropagation()}
      onMouseDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      className={cx('fixed z-50 flex flex-col overflow-hidden rounded-2xl border-[1.5px] border-ink bg-surface shadow-ink-lg', className)}
    >
      {children}
    </div>,
    document.body
  )
}
