import { AlertCircle, CheckCircle2, Info, X } from 'lucide-react'
import { useApp } from '../store/app'
import { cx } from './ui'

export function Toasts() {
  const { toasts, dismiss } = useApp()
  return (
    <div className="pointer-events-none fixed bottom-6 right-6 z-[60] flex w-[380px] flex-col gap-2.5">
      {toasts.map((t) => (
        <div
          key={t.id}
          role={t.kind === 'error' ? 'alert' : 'status'}
          className={cx(
            'pointer-events-auto flex items-start gap-3 rounded-2xl border-[1.5px] border-ink bg-surface px-4 py-3 shadow-ink-md',
            t.kind === 'error' && 'bg-bad-soft'
          )}
        >
          {t.kind === 'error' ? (
            <AlertCircle className="mt-0.5 size-[18px] shrink-0 text-bad-ink" />
          ) : t.kind === 'success' ? (
            <CheckCircle2 className="mt-0.5 size-[18px] shrink-0 text-ok-ink" />
          ) : (
            <Info className="mt-0.5 size-[18px] shrink-0 text-ink-2" />
          )}
          <div className="flex-1 text-sm leading-snug text-ink">
            {t.text}
            {t.action && (
              <button
                type="button"
                className="mt-1 block font-semibold text-accent-dark underline underline-offset-2"
                onClick={() => {
                  t.action!.run()
                  dismiss(t.id)
                }}
              >
                {t.action.label}
              </button>
            )}
          </div>
          <button type="button" aria-label="Tutup" onClick={() => dismiss(t.id)} className="text-muted hover:text-ink">
            <X className="size-4" />
          </button>
        </div>
      ))}
    </div>
  )
}
