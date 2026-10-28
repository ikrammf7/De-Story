import { useEffect, useRef, useState, type ButtonHTMLAttributes, type ReactNode } from 'react'
import { Loader2, X } from 'lucide-react'
import { create } from 'zustand'

import { cx } from './cx'
import { Popover } from './Popover'

export { cx }

type Variant = 'primary' | 'secondary' | 'accent' | 'ghost' | 'danger' | 'ink'
type Size = 'sm' | 'md' | 'lg'

const VARIANTS: Record<Variant, string> = {
  primary:
    'bg-accent text-white border-[1.5px] border-ink shadow-ink hover:bg-accent-dark active:translate-x-px active:translate-y-px active:shadow-none font-semibold',
  secondary: 'bg-surface text-ink border border-line-2 hover:border-ink-2 hover:bg-paper font-medium',
  accent: 'bg-surface text-accent-dark border-[1.5px] border-accent hover:bg-accent-soft font-semibold',
  ghost: 'bg-transparent text-ink-2 border border-transparent hover:bg-sand hover:text-ink font-medium',
  danger: 'bg-surface text-bad-ink border border-line-2 hover:border-bad-ink hover:bg-bad-soft font-medium',
  ink: 'bg-ink text-paper border border-ink hover:bg-ink-2 font-semibold'
}

const SIZES: Record<Size, string> = {
  sm: 'h-9 px-3 text-[13px] gap-1.5 rounded-[10px]',
  md: 'h-10 px-4 text-sm gap-2 rounded-xl',
  lg: 'h-12 px-6 text-[15px] gap-2.5 rounded-xl'
}

export function Button({
  variant = 'secondary',
  size = 'md',
  icon,
  loading,
  className,
  children,
  disabled,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; size?: Size; icon?: ReactNode; loading?: boolean }) {
  return (
    <button
      type="button"
      disabled={disabled || loading}
      className={cx(
        'inline-flex shrink-0 items-center justify-center whitespace-nowrap transition-colors disabled:cursor-not-allowed disabled:opacity-50',
        VARIANTS[variant],
        SIZES[size],
        className
      )}
      {...rest}
    >
      {loading ? <Loader2 className="size-4 animate-[spin_0.8s_linear_infinite]" /> : icon}
      {children}
    </button>
  )
}

export function IconButton({
  label,
  children,
  className,
  size = 40,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string; size?: number }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      style={{ width: size, height: size }}
      className={cx(
        'inline-flex shrink-0 items-center justify-center rounded-xl border border-line-2 bg-surface text-ink transition-colors hover:border-ink-2 hover:bg-paper disabled:opacity-40',
        className
      )}
      {...rest}
    >
      {children}
    </button>
  )
}

export function Chip({
  on,
  children,
  className,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { on?: boolean }) {
  return (
    <button
      type="button"
      aria-pressed={on}
      className={cx(
        'inline-flex h-9 items-center gap-1.5 whitespace-nowrap rounded-full border px-3.5 text-sm font-medium transition-colors',
        on ? 'border-ink bg-ink text-paper' : 'border-line-2 bg-surface text-ink hover:border-ink-2',
        className
      )}
      {...rest}
    >
      {children}
    </button>
  )
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  className,
  size = 'md'
}: {
  value: T
  options: { id: T; label: ReactNode; disabled?: boolean; title?: string }[]
  onChange: (v: T) => void
  className?: string
  size?: 'sm' | 'md'
}) {
  return (
    <div className={cx('inline-flex gap-0.5 rounded-xl border border-line bg-sand p-[3px]', className)}>
      {options.map((o) => (
        <button
          key={o.id}
          type="button"
          aria-pressed={o.id === value}
          disabled={o.disabled}
          title={o.title}
          onClick={() => onChange(o.id)}
          className={cx(
            'flex-1 whitespace-nowrap rounded-[9px] px-3 font-medium transition-colors disabled:cursor-not-allowed disabled:text-muted/70',
            size === 'sm' ? 'h-8 text-[13px]' : 'h-9 text-sm',
            o.id === value ? 'bg-surface text-ink shadow-[0_0_0_1px_var(--color-line)]' : 'text-ink-2 enabled:hover:text-ink'
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  )
}

type Tone = 'draft' | 'info' | 'warn' | 'ok' | 'bad' | 'ink'
const TONES: Record<Tone, string> = {
  draft: 'bg-draft text-ink-2',
  info: 'bg-info-soft text-info-ink',
  warn: 'bg-sun-soft text-sun-ink',
  ok: 'bg-ok-soft text-ok-ink',
  bad: 'bg-bad-soft text-bad-ink',
  ink: 'bg-ink text-paper'
}

export function Badge({ tone = 'draft', children, className }: { tone?: Tone; children: ReactNode; className?: string }) {
  return (
    <span className={cx('inline-flex items-center gap-1 whitespace-nowrap rounded-full px-2.5 py-[3px] text-xs font-semibold', TONES[tone], className)}>
      {children}
    </span>
  )
}

export function Switch({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={() => onChange(!on)}
      className={cx(
        'flex h-7 w-[46px] shrink-0 items-center rounded-full border-[1.5px] border-ink p-[3px] transition-colors',
        on ? 'justify-end bg-accent' : 'justify-start bg-line'
      )}
    >
      <span className="block size-[19px] rounded-full border-[1.5px] border-ink bg-white" />
    </button>
  )
}

export const inputCls =
  'w-full rounded-xl border border-line-2 bg-surface px-3.5 text-[15px] text-ink outline-none transition-colors placeholder:text-muted/80 focus:border-ink'

export function Field({ label, hint, children, className }: { label: ReactNode; hint?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <label className={cx('flex flex-col gap-1.5', className)}>
      <span className="text-sm font-semibold text-ink">{label}</span>
      {children}
      {hint && <span className="text-[13px] text-muted">{hint}</span>}
    </label>
  )
}

export function Progress({ value, className }: { value: number; className?: string }) {
  return (
    <span className={cx('block h-2 overflow-hidden rounded-full bg-[#EFE9DF]', className)}>
      <span className="block h-full rounded-full bg-accent transition-[width] duration-300" style={{ width: `${Math.round(value * 100)}%` }} />
    </span>
  )
}

export function Spinner({ className }: { className?: string }) {
  return <Loader2 className={cx('animate-[spin_0.8s_linear_infinite]', className ?? 'size-4')} />
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded-md border border-line bg-sand px-1.5 py-0.5 font-mono text-[11px] font-medium text-ink-2">{children}</kbd>
  )
}

export function Modal({
  open,
  onClose,
  title,
  subtitle,
  children,
  width = 600
}: {
  open: boolean
  onClose: () => void
  title: ReactNode
  subtitle?: ReactNode
  children: ReactNode
  width?: number
}) {
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open, onClose])
  if (!open) return null
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-ink/50 p-6" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        role="dialog"
        aria-modal="true"
        className="scroll-thin flex max-h-full flex-col gap-5 overflow-y-auto rounded-[20px] border-[1.5px] border-ink bg-surface p-7 shadow-[6px_6px_0_#1f1d1a]"
        style={{ width }}
      >
        <div className="flex items-start gap-3">
          <div className="flex-1">
            <h2 className="font-display text-[26px] font-bold leading-tight">{title}</h2>
            {subtitle && <p className="mt-1 text-sm text-ink-2">{subtitle}</p>}
          </div>
          <IconButton label="Tutup" onClick={onClose}>
            <X className="size-[18px]" />
          </IconButton>
        </div>
        {children}
      </div>
    </div>
  )
}

// ---------- confirm dialog ----------

interface ConfirmRequest {
  title: string
  body: string
  confirm: string
  /** Label of the other button; "Batal" by default. */
  cancel?: string
  danger?: boolean
  resolve: (ok: boolean) => void
}

const useConfirmStore = create<{ req: ConfirmRequest | null; set: (r: ConfirmRequest | null) => void }>((set) => ({
  req: null,
  set: (req) => set({ req })
}))

export function confirmDialog(opts: Omit<ConfirmRequest, 'resolve'>): Promise<boolean> {
  return new Promise((resolve) => useConfirmStore.getState().set({ ...opts, resolve }))
}

export function ConfirmHost() {
  const { req, set } = useConfirmStore()
  const close = (ok: boolean): void => {
    req?.resolve(ok)
    set(null)
  }
  return (
    <Modal open={!!req} onClose={() => close(false)} title={req?.title} width={480}>
      <p className="text-[15px] leading-relaxed text-ink-2">{req?.body}</p>
      <div className="flex justify-end gap-2.5">
        <Button onClick={() => close(false)}>{req?.cancel ?? 'Batal'}</Button>
        <Button variant={req?.danger ? 'ink' : 'primary'} onClick={() => close(true)} autoFocus>
          {req?.confirm}
        </Button>
      </div>
    </Modal>
  )
}

// ---------- popover menu ----------

/** A small action menu. It opens in a floating layer, so cards with hidden overflow cannot cut it off. */
export function Menu({ trigger, items }: { trigger: (open: () => void) => ReactNode; items: { label: string; icon?: ReactNode; danger?: boolean; run: () => void }[] }) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  return (
    <div ref={ref} className="relative inline-flex">
      {trigger(() => setOpen((o) => !o))}
      <Popover anchor={ref} open={open} onClose={() => setOpen(false)} width={210} align="right" maxHeight={320} className="p-1">
        {items.map((it) => (
          <button
            key={it.label}
            type="button"
            onClick={() => {
              setOpen(false)
              it.run()
            }}
            className={cx(
              'flex h-10 w-full shrink-0 items-center gap-2.5 rounded-xl px-3 text-left text-sm font-medium hover:bg-sand',
              it.danger ? 'text-bad-ink' : 'text-ink'
            )}
          >
            {it.icon}
            {it.label}
          </button>
        ))}
      </Popover>
    </div>
  )
}
