import { useEffect, useMemo, useRef } from 'react'
import { create } from 'zustand'
import { timelineChunks, type CaptionChunk } from '@shared/captions'
import type { Asset, Clip, Overlay } from '@shared/types'
import { useProject } from '../../../store/project'

export interface TimelineItem {
  clip: Clip
  index: number
  start: number
  end: number
  image: Asset | null
  video: Asset | null
  audio: Asset | null
}

interface PlaybackState {
  t: number
  playing: boolean
  seek(t: number): void
  play(): void
  pause(): void
  toggle(): void
}

export const usePlayback = create<PlaybackState>((set, get) => ({
  t: 0,
  playing: false,
  seek: (t) => set({ t: Math.max(0, t) }),
  play: () => set({ playing: true }),
  pause: () => set({ playing: false }),
  toggle: () => set({ playing: !get().playing })
}))

/** Which overlay the Overlay panel, the preview and the timeline have selected. */
export const useOverlayUi = create<{ selectedId: string | null; select(id: string | null): void }>((set) => ({
  selectedId: null,
  select: (id) => set({ selectedId: id })
}))

// Overlay edits read the store at call time, so rapid drags never work from a stale list.
const overlaysNow = (): Overlay[] => useProject.getState().project?.editor.overlays ?? []

export function addOverlay(o: Overlay): void {
  useProject.getState().updateEditor({ overlays: [...overlaysNow(), o] })
  useOverlayUi.getState().select(o.id)
}

export function updateOverlay(id: string, patch: Partial<Overlay>): void {
  useProject.getState().updateEditor({ overlays: overlaysNow().map((o) => (o.id === id ? ({ ...o, ...patch } as Overlay) : o)) })
}

export function removeOverlay(id: string): void {
  useProject.getState().updateEditor({ overlays: overlaysNow().filter((o) => o.id !== id) })
  if (useOverlayUi.getState().selectedId === id) useOverlayUi.getState().select(null)
}

export function useTimeline(): { items: TimelineItem[]; total: number; chunks: CaptionChunk[] } {
  const clips = useProject((s) => s.clips)
  const assets = useProject((s) => s.assets)
  return useMemo(() => {
    let acc = 0
    const items = clips.map((clip, index) => {
      const start = acc
      acc += clip.durationMs / 1000
      return {
        clip,
        index,
        start,
        end: acc,
        image: clip.imageAssetId ? (assets[clip.imageAssetId] ?? null) : null,
        video: clip.videoAssetId ? (assets[clip.videoAssetId] ?? null) : null,
        audio: clip.audioAssetId ? (assets[clip.audioAssetId] ?? null) : null
      }
    })
    const chunks = timelineChunks(items.map((it) => ({ start: it.start, words: it.audio?.meta.words ?? [] })))
    return { items, total: acc, chunks }
  }, [clips, assets])
}

export function itemAt(items: TimelineItem[], t: number): number {
  if (!items.length) return -1
  const i = items.findIndex((it) => t >= it.start && t < it.end)
  return i === -1 ? items.length - 1 : i
}

const dbToGain = (db: number): number => Math.pow(10, db / 20)

/** Drives the clock and keeps narration and music audio in step with it. */
export function usePlaybackEngine(items: TimelineItem[], total: number, music: Asset | null, musicDb: number, duck: boolean): void {
  const voices = useRef(new Map<string, HTMLAudioElement>())
  const musicEl = useRef<HTMLAudioElement | null>(null)

  useEffect(() => {
    const map = voices.current
    const keep = new Set<string>()
    for (const it of items) {
      if (!it.audio) continue
      keep.add(it.audio.id)
      if (!map.has(it.audio.id)) {
        const a = new Audio(it.audio.url)
        a.preload = 'auto'
        map.set(it.audio.id, a)
      }
    }
    for (const [id, a] of map) {
      if (!keep.has(id)) {
        a.pause()
        map.delete(id)
      }
    }
  }, [items])

  useEffect(() => {
    musicEl.current?.pause()
    if (!music) {
      musicEl.current = null
      return
    }
    const m = new Audio(music.url)
    m.loop = true
    m.preload = 'auto'
    musicEl.current = m
    return () => m.pause()
  }, [music?.id])

  useEffect(() => {
    let raf = 0
    let last = performance.now()
    const sync = (t: number, playing: boolean): void => {
      const idx = itemAt(items, t)
      let voiceActive = false
      items.forEach((it, i) => {
        if (!it.audio) return
        const a = voices.current.get(it.audio.id)
        if (!a) return
        const local = t - it.start
        const len = (it.audio.durationMs ?? it.clip.durationMs) / 1000
        if (playing && i === idx && local >= 0 && local < len) {
          voiceActive = true
          if (a.paused) {
            a.currentTime = local
            void a.play().catch(() => {})
          } else if (Math.abs(a.currentTime - local) > 0.3) a.currentTime = local
        } else if (!a.paused) a.pause()
      })
      const m = musicEl.current
      if (m) {
        m.volume = Math.min(1, dbToGain(musicDb) * (duck && voiceActive ? 0.35 : 1))
        if (playing) {
          if (m.paused) {
            m.currentTime = m.duration ? t % m.duration : t
            void m.play().catch(() => {})
          }
        } else if (!m.paused) m.pause()
      }
    }
    const tick = (now: number): void => {
      const s = usePlayback.getState()
      if (s.playing) {
        const t = s.t + (now - last) / 1000
        if (t >= total) usePlayback.setState({ t: total, playing: false })
        else usePlayback.setState({ t })
      }
      last = now
      const st = usePlayback.getState()
      sync(st.t, st.playing)
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => {
      cancelAnimationFrame(raf)
      for (const a of voices.current.values()) a.pause()
      musicEl.current?.pause()
    }
  }, [items, total, musicDb, duck])

  useEffect(() => () => usePlayback.setState({ playing: false }), [])
}
