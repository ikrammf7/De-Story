import { create } from 'zustand'
import type { Asset, Character, Clip, Job, Project, ProjectBundle, ProjectSnapshot } from '@shared/types'
import { errorText } from '../lib/format'

export type SaveState = 'saved' | 'dirty' | 'saving' | 'error'

interface ProjectState {
  loaded: boolean
  project: Project | null
  characters: Character[]
  clips: Clip[]
  assets: Record<string, Asset>
  jobs: Record<string, Job>
  saveState: SaveState
  saveError: string | null
  savedAt: number | null
  selectedClipId: string | null

  load(bundle: ProjectBundle): void
  unload(): void
  select(clipId: string | null): void
  updateProject(patch: Partial<Project>): void
  updateEditor(patch: Partial<Project['editor']>): void
  updateClip(id: string, patch: Partial<Clip>): void
  updateCharacter(id: string, patch: Partial<Character>): void
  addClip(afterId: string | null): string
  removeClip(id: string): void
  moveClip(id: string, dir: -1 | 1): void
  addCharacter(init?: { name: string; description: string }): string
  removeCharacter(id: string): void
  save(): Promise<void>
  flush(): Promise<void>

  applyClip(clipId: string, patch: Partial<Clip>): void
  applyCharacter(characterId: string, patch: Partial<Character>): void
  addAsset(asset: Asset): void
  upsertJob(job: Job): void
}

const AUTOSAVE_MS = 1200
let timer: ReturnType<typeof setTimeout> | null = null
let inFlight: Promise<void> | null = null
let changedDuringSave = false

function snapshot(s: ProjectState): ProjectSnapshot {
  const { createdAt: _c, updatedAt: _u, coverAssetId: _cv, ...project } = s.project!
  return {
    project,
    characters: s.characters.map(({ sheetAssetId: _s, ...c }) => c),
    clips: s.clips.map(({ imageAssetId: _i, videoAssetId: _v, audioAssetId: _a, ...c }) => c)
  }
}

const newId = (): string => crypto.randomUUID()

export const useProject = create<ProjectState>((set, get) => {
  const touch = (): void => {
    if (get().saveState === 'saving') changedDuringSave = true
    set({ saveState: get().saveState === 'saving' ? 'saving' : 'dirty' })
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => void get().save(), AUTOSAVE_MS)
  }
  const resort = (clips: Clip[]): Clip[] => clips.map((c, i) => ({ ...c, sort: i }))

  return {
    loaded: false,
    project: null,
    characters: [],
    clips: [],
    assets: {},
    jobs: {},
    saveState: 'saved',
    saveError: null,
    savedAt: null,
    selectedClipId: null,

    load: (b) =>
      set({
        loaded: true,
        project: b.project,
        characters: b.characters,
        clips: b.clips,
        assets: Object.fromEntries(b.assets.map((a) => [a.id, a])),
        jobs: Object.fromEntries(b.jobs.map((j) => [j.id, j])),
        saveState: 'saved',
        saveError: null,
        savedAt: b.project.updatedAt,
        selectedClipId: get().selectedClipId && b.clips.some((c) => c.id === get().selectedClipId) ? get().selectedClipId : (b.clips[0]?.id ?? null)
      }),
    unload: () => {
      if (timer) clearTimeout(timer)
      set({ loaded: false, project: null, characters: [], clips: [], assets: {}, jobs: {}, selectedClipId: null, saveState: 'saved' })
    },
    select: (clipId) => set({ selectedClipId: clipId }),

    updateProject: (patch) => {
      set({ project: { ...get().project!, ...patch } })
      touch()
    },
    updateEditor: (patch) => {
      const p = get().project!
      set({ project: { ...p, editor: { ...p.editor, ...patch } } })
      touch()
    },
    updateClip: (id, patch) => {
      set({ clips: get().clips.map((c) => (c.id === id ? { ...c, ...patch } : c)) })
      touch()
    },
    updateCharacter: (id, patch) => {
      set({ characters: get().characters.map((c) => (c.id === id ? { ...c, ...patch } : c)) })
      touch()
    },
    addClip: (afterId) => {
      const p = get().project!
      const clips = [...get().clips]
      const at = afterId ? clips.findIndex((c) => c.id === afterId) + 1 : clips.length
      const clip: Clip = {
        id: newId(),
        projectId: p.id,
        sort: at,
        title: 'Adegan baru',
        story: '',
        visualPrompt: '',
        visualSource: null,
        narration: '',
        durationMs: 5000,
        characterIds: [],
        motionType: 'camera',
        cameraPreset: 'zoomin',
        motionStrength: 'halus',
        videoPrompt: '',
        transition: 'fade',
        imageAssetId: null,
        videoAssetId: null,
        audioAssetId: null
      }
      clips.splice(at, 0, clip)
      set({ clips: resort(clips), selectedClipId: clip.id })
      touch()
      return clip.id
    },
    removeClip: (id) => {
      const clips = get().clips
      const i = clips.findIndex((c) => c.id === id)
      const rest = resort(clips.filter((c) => c.id !== id))
      set({ clips: rest, selectedClipId: rest[Math.min(i, rest.length - 1)]?.id ?? null })
      touch()
    },
    moveClip: (id, dir) => {
      const clips = [...get().clips]
      const i = clips.findIndex((c) => c.id === id)
      const j = i + dir
      if (i < 0 || j < 0 || j >= clips.length) return
      ;[clips[i], clips[j]] = [clips[j], clips[i]]
      set({ clips: resort(clips) })
      touch()
    },
    addCharacter: (init) => {
      const p = get().project!
      const c: Character = {
        id: newId(),
        projectId: p.id,
        name: init?.name.trim() || 'Pemeran baru',
        description: init?.description.trim() ?? '',
        sheetAssetId: null,
        sort: get().characters.length
      }
      set({ characters: [...get().characters, c] })
      touch()
      return c.id
    },
    removeCharacter: (id) => {
      set({
        characters: get().characters.filter((c) => c.id !== id),
        clips: get().clips.map((c) => ({ ...c, characterIds: c.characterIds.filter((x) => x !== id) }))
      })
      touch()
    },

    save: async () => {
      if (timer) {
        clearTimeout(timer)
        timer = null
      }
      if (!get().project) return
      if (inFlight) {
        changedDuringSave = true
        return inFlight
      }
      if (get().saveState === 'saved') return
      changedDuringSave = false
      set({ saveState: 'saving', saveError: null })
      inFlight = (async () => {
        try {
          const { updatedAt } = await window.api.projects.save(snapshot(get()))
          set({ savedAt: updatedAt, saveState: changedDuringSave ? 'dirty' : 'saved' })
        } catch (e) {
          set({ saveState: 'error', saveError: errorText(e) })
        } finally {
          inFlight = null
        }
        if (changedDuringSave) {
          changedDuringSave = false
          timer = setTimeout(() => void get().save(), 300)
        }
      })()
      return inFlight
    },
    flush: async () => {
      let retried = false
      for (;;) {
        if (inFlight) await inFlight
        const state = get()
        if (state.saveState === 'saved' || !state.project) return
        if (state.saveState === 'error') {
          if (retried) throw new Error(state.saveError ?? 'Gagal menyimpan proyek.')
          retried = true
        }
        if (state.saveState === 'dirty' || state.saveState === 'error') await get().save()
      }
    },

    applyClip: (clipId, patch) => set({ clips: get().clips.map((c) => (c.id === clipId ? { ...c, ...patch } : c)) }),
    applyCharacter: (characterId, patch) =>
      set({ characters: get().characters.map((c) => (c.id === characterId ? { ...c, ...patch } : c)) }),
    addAsset: (asset) => {
      if (asset.projectId !== get().project?.id) return
      set({ assets: { ...get().assets, [asset.id]: asset } })
    },
    upsertJob: (job) => {
      if (job.projectId !== get().project?.id) return
      set({ jobs: { ...get().jobs, [job.id]: job } })
    }
  }
})

/** Latest active or recent job of a kind for a clip or character. */
export function jobFor(jobs: Record<string, Job>, kind: Job['kind'], target: { clipId?: string; characterId?: string }): Job | null {
  let best: Job | null = null
  for (const j of Object.values(jobs)) {
    if (j.kind !== kind) continue
    if (target.clipId && j.clipId !== target.clipId) continue
    if (target.characterId && j.characterId !== target.characterId) continue
    if (!best || j.createdAt > best.createdAt) best = j
  }
  return best
}

export const isRunning = (j: Job | null | undefined): boolean => !!j && (j.status === 'queued' || j.status === 'running')
