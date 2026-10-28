import { useRef, useState } from 'react'
import { Blend, ChevronsLeft, Maximize2, Scissors } from 'lucide-react'
import { TRANSITIONS } from '@shared/motion'
import type { Clip, TransitionId } from '@shared/types'
import { Popover } from '../../../components/Popover'
import { cx } from '../../../components/ui'
import { useProject } from '../../../store/project'

const ICONS: Record<TransitionId, typeof Blend> = { fade: Blend, slideleft: ChevronsLeft, zoomin: Maximize2, cut: Scissors }

const NOTES: Record<TransitionId, string> = {
  fade: 'Tenang, cocok untuk pergantian waktu',
  slideleft: 'Pindah tempat atau sudut pandang',
  zoomin: 'Dramatis, untuk momen penting',
  cut: 'Langsung, cocok untuk adegan cepat'
}

/** The joint between a clip and the next one on the timeline; it opens a picker for that clip's transition. */
export function TransitionButton({ clip, next, x }: { clip: Clip; next: Clip; x: number }) {
  const { clips, updateClip } = useProject()
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLButtonElement>(null)
  const current = TRANSITIONS.find((t) => t.id === clip.transition) ?? TRANSITIONS[0]
  const Icon = ICONS[current.id]
  const joints = clips.slice(0, -1)
  const allSame = joints.every((c) => c.transition === clip.transition)

  return (
    <>
      <button
        ref={ref}
        type="button"
        onPointerDown={(e) => e.stopPropagation()}
        onClick={() => setOpen((v) => !v)}
        aria-label={`Transisi dari "${clip.title}" ke "${next.title}": ${current.label}`}
        aria-expanded={open}
        title={`Transisi: ${current.label}`}
        className={cx(
          'absolute top-1/2 z-[2] flex size-7 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border-[1.5px] border-ink shadow-[0_1px_0_rgba(0,0,0,0.25)] transition-colors',
          open ? 'bg-accent text-white' : current.id === 'cut' ? 'bg-surface text-ink-2 hover:bg-sun-soft' : 'bg-sun text-ink hover:bg-sun-soft'
        )}
        style={{ left: x }}
      >
        <Icon className="size-3.5" strokeWidth={2.4} />
      </button>
      <Popover anchor={ref} open={open} onClose={() => setOpen(false)} width={290} maxHeight={380} label="Pilih transisi" className="p-1.5">
        <p className="px-2.5 pb-1.5 pt-1 text-xs text-muted">
          Transisi {clip.title} → {next.title}
        </p>
        {TRANSITIONS.map((t) => {
          const I = ICONS[t.id]
          const on = t.id === clip.transition
          return (
            <button
              key={t.id}
              type="button"
              aria-pressed={on}
              onClick={() => {
                updateClip(clip.id, { transition: t.id })
                setOpen(false)
              }}
              className={cx('flex w-full items-center gap-3 rounded-xl px-2.5 py-2 text-left', on ? 'bg-sun-soft' : 'hover:bg-sand')}
            >
              <span className={cx('flex size-8 shrink-0 items-center justify-center rounded-full border', on ? 'border-ink bg-sun' : 'border-line-2 bg-surface')}>
                <I className="size-4" />
              </span>
              <span className="flex min-w-0 flex-col">
                <span className={cx('text-sm', on && 'font-semibold')}>{t.label}</span>
                <span className="text-xs text-muted">{NOTES[t.id]}</span>
              </span>
            </button>
          )
        })}
        {!allSame && (
          <button
            type="button"
            onClick={() => {
              joints.forEach((c) => c.transition !== clip.transition && updateClip(c.id, { transition: clip.transition }))
              setOpen(false)
            }}
            className="mt-1 w-full border-t border-line px-2.5 pb-1 pt-2.5 text-left text-xs font-semibold text-accent-dark hover:underline"
          >
            Pakai “{current.label}” untuk semua sambungan
          </button>
        )}
      </Popover>
    </>
  )
}
