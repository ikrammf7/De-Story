import { memo, useLayoutEffect, useRef, useState, type PointerEvent } from 'react'
import { Film, Image as ImageIcon, Layers, Mic, Music, Type, ZoomIn } from 'lucide-react'
import type { CaptionChunk } from '@shared/captions'
import { overlayEnd } from '@shared/overlays'
import type { Asset, Overlay } from '@shared/types'
import { cx } from '../../../components/ui'
import { usePlayback, type TimelineItem } from './engine'
import { TransitionButton } from './TransitionButton'
import { Waveform } from './Waveform'

const LABEL_W = 140
const TICK_STEPS = [1, 2, 5, 10, 15, 30, 60, 120]

function Playhead({ pps }: { pps: number }) {
  const t = usePlayback((s) => s.t)
  return (
    <div className="pointer-events-none absolute bottom-0 top-0 z-10 w-0.5 bg-accent" style={{ left: t * pps }}>
      <span className="absolute -left-[6px] -top-0.5 size-3.5 rounded-[3px] border-[1.5px] border-ink bg-accent" />
    </div>
  )
}

const Tracks = memo(function Tracks({
  items,
  chunks,
  music,
  total,
  pps,
  selectedId,
  onSelect,
  overlays,
  assets,
  selectedOverlayId,
  onSelectOverlay
}: {
  items: TimelineItem[]
  chunks: CaptionChunk[]
  music: Asset | null
  total: number
  pps: number
  selectedId: string | null
  onSelect: (clipId: string, start: number) => void
  overlays: Overlay[]
  assets: Record<string, Asset>
  selectedOverlayId: string | null
  onSelectOverlay: (id: string, start: number) => void
}) {
  const step = TICK_STEPS.find((s) => s * pps >= 70) ?? 120
  const ticks: number[] = []
  for (let s = 0; s <= total + 0.001; s += step) ticks.push(s)
  return (
    <>
      <div className="relative h-[22px] border-b border-line">
        {ticks.map((s) => (
          <span
            key={s}
            className="absolute top-0 h-[21px] border-l border-line-2 pl-1 font-mono text-[11px] text-muted"
            style={{ left: s * pps }}
          >
            {Math.floor(s / 60)}:{String(Math.round(s % 60)).padStart(2, '0')}
          </span>
        ))}
      </div>
      <div className="relative h-16">
        {items.map((it) => (
          <button
            key={it.clip.id}
            type="button"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={() => onSelect(it.clip.id, it.start)}
            aria-label={`Klip ${it.index + 1} ${it.clip.title}`}
            aria-pressed={it.clip.id === selectedId}
            className={cx(
              'absolute top-0 flex h-16 flex-col justify-between overflow-hidden rounded-lg bg-sand bg-cover bg-center p-1.5 text-left',
              it.clip.id === selectedId ? 'border-2 border-accent' : 'border border-ink'
            )}
            style={{
              left: it.start * pps + 1,
              width: Math.max(8, (it.end - it.start) * pps - 3),
              backgroundImage: it.image ? `url("${it.image.url}")` : undefined
            }}
          >
            <span className="flex items-center gap-1 self-start">
              <span className="rounded-[5px] border border-ink bg-surface px-1 font-mono text-[11px]">{String(it.index + 1).padStart(2, '0')}</span>
              {it.clip.motionType === 'video' && it.video && (
                <span className="flex size-[18px] items-center justify-center rounded-[5px] bg-ink text-paper">
                  <Film className="size-3" />
                </span>
              )}
            </span>
            <span className="max-w-full truncate rounded bg-surface/85 px-1 text-xs font-semibold">{it.clip.title}</span>
          </button>
        ))}
        {/* One button on each joint: the transition belongs to the clip before it. */}
        {items.slice(1).map((it, i) => (
          <TransitionButton key={`tr-${items[i].clip.id}`} clip={items[i].clip} next={it.clip} x={it.start * pps} />
        ))}
      </div>
      <div className="relative h-[30px]">
        {chunks.map((c, i) => (
          <span
            key={i}
            className="absolute top-0 flex h-[30px] items-center overflow-hidden whitespace-nowrap rounded-md border border-[#E3C46A] bg-sun-soft px-1.5 text-xs text-sun-ink"
            style={{ left: c.start * pps + 1, width: Math.max(4, (c.end - c.start) * pps - 2) }}
            title={c.words.map((w) => w.text).join(' ')}
          >
            {c.words.map((w) => w.text).join(' ')}
          </span>
        ))}
      </div>
      <div className="relative h-10">
        {items.map((it) =>
          it.audio ? (
            <span
              key={it.audio.id}
              className="absolute top-0 flex h-10 items-center overflow-hidden rounded-md border border-line-2 bg-[#EFEAE2] px-1"
              style={{ left: it.start * pps + 1, width: Math.max(4, ((it.audio.durationMs ?? 0) / 1000) * pps - 2) }}
            >
              <Waveform url={it.audio.url} className="h-8 w-full" />
            </span>
          ) : null
        )}
      </div>
      <div className="relative h-[30px]">
        {music && (
          <span
            className="absolute left-px top-0 flex h-[30px] items-center gap-2 overflow-hidden whitespace-nowrap rounded-md border border-[#9CC596] bg-ok-soft px-2.5 text-xs text-ok-ink"
            style={{ width: Math.max(10, total * pps - 2) }}
          >
            <Music className="size-3.5" />
            {music.prompt ?? 'Musik latar'}
          </span>
        )}
      </div>
      <div className="relative h-[30px]">
        {overlays.map((o) => {
          const start = Math.min(o.start, total)
          const end = overlayEnd(o, total)
          if (end <= start) return null
          const on = o.id === selectedOverlayId
          const label = o.kind === 'text' ? o.text.split('\n')[0] || 'Teks' : (assets[o.assetId]?.prompt ?? 'Gambar')
          return (
            <button
              key={o.id}
              type="button"
              onPointerDown={(e) => e.stopPropagation()}
              onClick={() => onSelectOverlay(o.id, start)}
              aria-pressed={on}
              title={label}
              className={cx(
                'absolute top-0 flex h-[30px] items-center gap-1.5 overflow-hidden whitespace-nowrap rounded-md px-2 text-xs font-medium',
                o.kind === 'image' ? 'bg-info-soft text-info-ink' : 'bg-accent-soft text-accent-dark',
                on ? 'z-[1] border-2 border-accent' : 'border border-line-3'
              )}
              style={{ left: start * pps + 1, width: Math.max(10, (end - start) * pps - 2) }}
            >
              {o.kind === 'image' ? <ImageIcon className="size-3.5 shrink-0" /> : <Type className="size-3.5 shrink-0" />}
              <span className="truncate">{label}</span>
            </button>
          )
        })}
      </div>
    </>
  )
})

export function Timeline({
  items,
  chunks,
  music,
  total,
  selectedId,
  onSelect,
  overlays,
  assets,
  selectedOverlayId,
  onSelectOverlay
}: {
  items: TimelineItem[]
  chunks: CaptionChunk[]
  music: Asset | null
  total: number
  selectedId: string | null
  onSelect: (clipId: string) => void
  overlays: Overlay[]
  assets: Record<string, Asset>
  selectedOverlayId: string | null
  onSelectOverlay: (id: string) => void
}) {
  const scroller = useRef<HTMLDivElement>(null)
  const [width, setWidth] = useState(1000)
  const [zoom, setZoom] = useState(1)
  const seek = usePlayback((s) => s.seek)

  useLayoutEffect(() => {
    const el = scroller.current
    if (!el) return
    const ro = new ResizeObserver(() => setWidth(el.clientWidth))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  const pps = Math.max(2, ((width - 16) / Math.max(total, 1)) * zoom)

  const seekFrom = (e: PointerEvent<HTMLDivElement>): void => {
    const el = scroller.current!
    const x = e.clientX - el.getBoundingClientRect().left + el.scrollLeft - 8
    seek(Math.min(total, Math.max(0, x / pps)))
  }

  return (
    <section className="flex min-h-0 flex-col border-t border-line-2 bg-surface">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-line px-4 text-[13px] text-ink-2">
        <span>Klik klip atau overlay untuk mengaturnya. Klik tombol bulat di antara klip untuk memilih transisinya.</span>
        <span className="flex-1" />
        <label className="flex items-center gap-2">
          <ZoomIn className="size-4" />
          Zoom
          <input type="range" min={1} max={8} step={0.25} value={zoom} onChange={(e) => setZoom(Number(e.target.value))} aria-label="Zoom timeline" className="w-32" />
        </label>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-[140px_minmax(0,1fr)] py-1.5 pr-4">
        <div className="flex flex-col gap-2 pt-[30px] text-[13px] font-semibold" style={{ width: LABEL_W }}>
          <span className="flex h-16 items-center gap-2 pl-4">
            <ImageIcon className="size-4" />
            Klip
          </span>
          <span className="flex h-[30px] items-center gap-2 pl-4">
            <Type className="size-4" />
            Caption
          </span>
          <span className="flex h-10 items-center gap-2 pl-4">
            <Mic className="size-4" />
            Suara TTS
          </span>
          <span className="flex h-[30px] items-center gap-2 pl-4">
            <Music className="size-4" />
            Musik
          </span>
          <span className="flex h-[30px] items-center gap-2 pl-4">
            <Layers className="size-4" />
            Overlay
          </span>
        </div>
        <div
          ref={scroller}
          className="scroll-thin relative overflow-x-auto overflow-y-hidden"
          onPointerDown={(e) => {
            e.currentTarget.setPointerCapture(e.pointerId)
            seekFrom(e)
          }}
          onPointerMove={(e) => e.buttons === 1 && seekFrom(e)}
        >
          <div className="relative flex flex-col gap-2 pl-2" style={{ width: total * pps + 16 }}>
            <Tracks
              items={items}
              chunks={chunks}
              music={music}
              total={total}
              pps={pps}
              selectedId={selectedId}
              onSelect={(id, start) => {
                onSelect(id)
                seek(start)
              }}
              overlays={overlays}
              assets={assets}
              selectedOverlayId={selectedOverlayId}
              onSelectOverlay={(id, start) => {
                onSelectOverlay(id)
                seek(start + 0.01)
              }}
            />
            <div className="pointer-events-none absolute inset-y-0 left-2 right-0">
              <Playhead pps={pps} />
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}
