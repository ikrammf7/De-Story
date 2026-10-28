import { useLayoutEffect, useRef, type TextareaHTMLAttributes } from 'react'
import { cx } from './ui'

/**
 * A textarea that grows with its text (up to `maxHeight`, then scrolls) and can still be resized by hand.
 * It never shrinks inside a scrolling flex column, where a plain textarea can collapse to one line.
 */
export function AutoTextarea({
  minRows = 3,
  maxHeight = 340,
  className,
  value,
  ...rest
}: TextareaHTMLAttributes<HTMLTextAreaElement> & { minRows?: number; maxHeight?: number }) {
  const ref = useRef<HTMLTextAreaElement>(null)

  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    el.style.height = 'auto'
    const border = el.offsetHeight - el.clientHeight
    el.style.height = `${Math.min(el.scrollHeight + border, maxHeight)}px`
  }, [value, maxHeight])

  return <textarea ref={ref} rows={minRows} value={value} className={cx('shrink-0 resize-y', className)} {...rest} />
}
