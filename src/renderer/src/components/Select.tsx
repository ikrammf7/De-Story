import { useEffect, useId, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode } from 'react'
import { ChevronDown, Search } from 'lucide-react'
import { Popover } from './Popover'
import { cx } from './ui'

export interface SelectOption<T extends string | number> {
  value: T
  label: string
  /** Muted text after the label, e.g. a voice's tone. */
  hint?: string
  /** Shown before the label, in the list and on the button. */
  icon?: ReactNode
  /** Options sharing a group are listed under one heading. */
  group?: string
  disabled?: boolean
  /** A second line under the label, e.g. why an option cannot be picked. */
  note?: string
  /** Style for the label text, e.g. a font preview. */
  labelStyle?: CSSProperties
}

const SIZES = { sm: 'h-10 text-sm', md: 'h-11 text-[15px]', lg: 'h-12 text-[15px]' }

/** The app's dropdown: a styled replacement for the native select, with keyboard support and optional search. */
export function Select<T extends string | number>({
  value,
  options,
  onChange,
  label,
  variant = 'field',
  size = 'md',
  searchable,
  menuWidth,
  align,
  disabled,
  className,
  renderAction
}: {
  value: T
  options: SelectOption<T>[]
  onChange: (value: T) => void
  /** Accessible name of the control. */
  label: string
  /** "field" matches text inputs; "pill" is the rounded chip used in the Home composer. */
  variant?: 'field' | 'pill'
  size?: keyof typeof SIZES
  /** Adds a search box. On by default for long lists. */
  searchable?: boolean
  menuWidth?: number
  align?: 'left' | 'right'
  disabled?: boolean
  className?: string
  /** A control at the end of each row (e.g. a play button); clicking it does not pick the option. */
  renderAction?: (o: SelectOption<T>) => ReactNode
}) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [active, setActive] = useState(0)
  const trigger = useRef<HTMLButtonElement>(null)
  const listId = useId()
  const search = searchable ?? options.length > 12
  const current = options.find((o) => o.value === value)

  const visible = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean)
    if (!words.length) return options
    return options.filter((o) => {
      const hay = `${o.label} ${o.hint ?? ''} ${o.group ?? ''}`.toLowerCase()
      return words.every((w) => hay.includes(w))
    })
  }, [options, query])

  useEffect(() => {
    if (open) document.getElementById(`${listId}-${active}`)?.scrollIntoView({ block: 'nearest' })
  }, [open, active, listId])

  const show = (): void => {
    setQuery('')
    setActive(Math.max(0, options.findIndex((o) => o.value === value)))
    setOpen(true)
  }

  const hide = (refocus: boolean): void => {
    setOpen(false)
    if (refocus) trigger.current?.focus()
  }

  const choose = (o: SelectOption<T> | undefined): void => {
    if (!o || o.disabled) return
    onChange(o.value)
    hide(true)
  }

  /** Next enabled row in a direction, wrapping around. */
  const step = (from: number, dir: 1 | -1): number => {
    for (let n = 1; n <= visible.length; n++) {
      const i = (from + dir * n + visible.length) % visible.length
      if (!visible[i].disabled) return i
    }
    return from
  }

  const onKey = (e: KeyboardEvent<HTMLElement>): void => {
    if (!open) {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
        e.preventDefault()
        show()
      }
      return
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      setActive((a) => step(a, e.key === 'ArrowDown' ? 1 : -1))
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault()
      setActive(step(e.key === 'Home' ? -1 : 0, e.key === 'Home' ? 1 : -1))
    } else if (e.key === 'Enter' || (e.key === ' ' && !search)) {
      e.preventDefault()
      choose(visible[active])
    } else if (e.key === 'Tab') {
      hide(false)
    } else if (!search && e.key.length === 1 && /\S/.test(e.key)) {
      // Type-ahead: jump to the next option starting with the typed letter.
      const k = e.key.toLowerCase()
      for (let n = 1; n <= visible.length; n++) {
        const i = (active + n) % visible.length
        if (!visible[i].disabled && visible[i].label.toLowerCase().startsWith(k)) {
          setActive(i)
          break
        }
      }
    }
  }

  const activeId = open && visible[active] ? `${listId}-${active}` : undefined

  return (
    <>
      <button
        ref={trigger}
        type="button"
        disabled={disabled}
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-activedescendant={search ? undefined : activeId}
        onClick={() => (open ? hide(false) : show())}
        onKeyDown={onKey}
        className={cx(
          'flex min-w-0 items-center text-left transition-colors disabled:cursor-not-allowed disabled:opacity-50',
          variant === 'pill'
            ? 'inline-flex h-10 gap-2 rounded-full border bg-paper pl-3.5 pr-3 text-sm font-medium'
            : cx('w-full gap-2.5 rounded-xl border bg-surface px-3.5 text-ink', SIZES[size]),
          open ? 'border-ink' : 'border-line-2 hover:border-ink-2',
          className
        )}
      >
        {current?.icon}
        <span className={cx('truncate', variant === 'field' && 'flex-1')} style={current?.labelStyle}>
          {current?.label ?? String(value)}
          {current?.hint && variant === 'field' && <span className="text-muted"> · {current.hint}</span>}
        </span>
        <ChevronDown className={cx('size-4 shrink-0 text-ink-2 transition-transform', variant === 'pill' && 'size-3.5', open && 'rotate-180')} />
      </button>

      <Popover anchor={trigger} open={open} onClose={(why) => hide(why === 'escape')} width={menuWidth} align={align} maxHeight={search ? 420 : 360}>
        {search && (
          <div className="border-b border-line p-2">
            <label className="flex h-9 items-center gap-2 rounded-xl border border-line-2 bg-paper px-3 focus-within:border-ink">
              <Search className="size-4 shrink-0 text-muted" />
              <input
                autoFocus
                value={query}
                onChange={(e) => {
                  setQuery(e.target.value)
                  setActive(0)
                }}
                onKeyDown={onKey}
                placeholder="Cari…"
                aria-label={`Cari ${label.toLowerCase()}`}
                aria-controls={listId}
                aria-activedescendant={activeId}
                className="flex-1 bg-transparent text-sm outline-none"
              />
            </label>
          </div>
        )}
        <div id={listId} role="listbox" aria-label={label} className="scroll-thin min-h-0 flex-1 overflow-y-auto p-1.5">
          {visible.map((o, i) => {
            const selected = o.value === value
            const heading = o.group && o.group !== visible[i - 1]?.group
            return (
              <div key={String(o.value)}>
                {heading && <p className="px-3 pb-1 pt-2.5 text-[11px] font-semibold uppercase tracking-[0.06em] text-muted">{o.group}</p>}
                <div
                  id={`${listId}-${i}`}
                  role="option"
                  aria-selected={selected}
                  aria-disabled={o.disabled || undefined}
                  onMouseDown={(e) => e.preventDefault()}
                  onMouseEnter={() => !o.disabled && setActive(i)}
                  onClick={() => choose(o)}
                  className={cx(
                    'flex items-center gap-2.5 rounded-xl px-3 py-2 text-sm',
                    o.disabled ? 'cursor-not-allowed text-muted' : cx('cursor-pointer', selected ? 'bg-sun-soft' : i === active && 'bg-sand')
                  )}
                >
                  {o.icon && <span className={cx('flex shrink-0', o.disabled && 'opacity-50')}>{o.icon}</span>}
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className={cx('truncate', selected && !o.labelStyle && 'font-semibold')} style={o.labelStyle}>
                      {o.label}
                    </span>
                    {o.note && <span className="text-xs text-muted">{o.note}</span>}
                  </span>
                  {o.hint && <span className="shrink-0 text-xs text-muted">{o.hint}</span>}
                  {renderAction && (
                    <span
                      className="flex shrink-0"
                      onMouseDown={(e) => {
                        e.preventDefault()
                        e.stopPropagation()
                      }}
                      onClick={(e) => e.stopPropagation()}
                    >
                      {renderAction(o)}
                    </span>
                  )}
                </div>
              </div>
            )
          })}
          {!visible.length && <p className="px-3 py-3 text-sm text-muted">Tidak ada yang cocok.</p>}
        </div>
      </Popover>
    </>
  )
}
