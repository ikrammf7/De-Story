import { useEffect, useState } from 'react'
import { cx } from '../../components/ui'
import { useProject } from '../../store/project'
import { useOverlayUi, usePlayback, usePlaybackEngine, useTimeline } from './editor/engine'
import { INSPECTOR_TABS, Inspector, type InspectorTab } from './editor/Inspector'
import { Player } from './editor/Player'
import { Timeline } from './editor/Timeline'

export function EditorStep() {
  const { project, assets, selectedClipId, select, clips } = useProject()
  const { items, total, chunks } = useTimeline()
  const [tab, setTab] = useState<InspectorTab>('klip')
  const selectedOverlayId = useOverlayUi((s) => s.selectedId)
  const selectOverlay = useOverlayUi((s) => s.select)
  const music = project?.editor.musicAssetId ? (assets[project.editor.musicAssetId] ?? null) : null
  usePlaybackEngine(items, total, music, project?.editor.musicVolumeDb ?? -18, project?.editor.duckMusic ?? true)

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      const tag = (e.target as HTMLElement)?.tagName
      if (e.code === 'Space' && tag !== 'INPUT' && tag !== 'TEXTAREA' && tag !== 'SELECT') {
        e.preventDefault()
        const s = usePlayback.getState()
        if (!s.playing && s.t >= total - 0.05) s.seek(0)
        s.toggle()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [total])

  if (!project) return null
  const selIndex = clips.findIndex((c) => c.id === selectedClipId)
  const sel = selIndex >= 0 ? clips[selIndex] : null

  return (
    <div className="grid h-full grid-rows-[minmax(0,1fr)_330px]">
      <div className="grid min-h-0 grid-cols-[84px_minmax(0,1fr)_360px]">
        <nav aria-label="Panel editor" className="flex flex-col items-center gap-1 border-r border-line bg-surface py-3">
          {INSPECTOR_TABS.map((t) => {
            const Icon = t.icon
            const on = tab === t.id
            return (
              <button
                key={t.id}
                type="button"
                aria-pressed={on}
                onClick={() => setTab(t.id)}
                className={cx(
                  'flex h-[62px] w-[68px] flex-col items-center justify-center gap-1 rounded-xl text-xs',
                  on ? 'border-[1.5px] border-ink bg-sun-soft font-semibold text-ink' : 'font-medium text-ink-2 hover:bg-sand'
                )}
              >
                <Icon className="size-5" />
                {t.label}
              </button>
            )
          })}
        </nav>
        <Player
          items={items}
          total={total}
          chunks={chunks}
          aspect={project.aspectRatio}
          editor={project.editor}
          assets={assets}
          editOverlays={tab === 'overlay'}
        />
        <Inspector tab={tab} clip={sel} index={selIndex} total={total} />
      </div>
      <Timeline
        items={items}
        chunks={project.editor.captionStyle === 'none' ? [] : chunks}
        music={music}
        total={total}
        selectedId={selectedClipId}
        onSelect={(id) => {
          select(id)
          setTab('klip')
        }}
        overlays={project.editor.overlays ?? []}
        assets={assets}
        selectedOverlayId={selectedOverlayId}
        onSelectOverlay={(id) => {
          selectOverlay(id)
          setTab('overlay')
        }}
      />
    </div>
  )
}
