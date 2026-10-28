import type { Clip } from '@shared/types'
import { confirmDialog } from '../components/ui'
import { useApp } from '../store/app'
import { isRunning, jobFor, useProject } from '../store/project'
import { errorText } from './format'

/**
 * The project's narration is always voiced as one continuous take for every scene, then cut per scene,
 * so the voice and intonation stay the same throughout. This hook drives that single action.
 */
export function useNarration(): {
  running: boolean
  progress: number
  message: string | null
  /** Scenes with narration that have no voice yet, or whose text changed after voicing. */
  stale: number
  voiced: number
  start(): Promise<void>
} {
  const project = useProject((s) => s.project)
  const clips = useProject((s) => s.clips)
  const assets = useProject((s) => s.assets)
  const jobs = useProject((s) => s.jobs)
  const flush = useProject((s) => s.flush)
  const { toast } = useApp()
  const job = jobFor(jobs, 'narration', {})
  const outdated = (c: Clip): boolean => {
    if (!c.narration.trim()) return false
    const voice = c.audioAssetId ? assets[c.audioAssetId] : null
    return !voice || (voice.prompt ?? '').trim() !== c.narration.trim()
  }
  const voiced = clips.filter((c) => c.audioAssetId).length

  const start = async (): Promise<void> => {
    if (!project) return
    if (voiced > 0) {
      const ok = await confirmDialog({
        title: 'Rekam ulang suara narasi?',
        body: 'Narasi semua adegan direkam ulang dalam satu kali jalan lalu dipotong per adegan, supaya suaranya tetap konsisten. Suara lama tetap tersimpan di riwayat.',
        confirm: 'Rekam ulang'
      })
      if (!ok) return
    }
    try {
      await flush()
      await window.api.generate.narration(project.id, 'all')
    } catch (e) {
      toast('error', errorText(e))
    }
  }

  return {
    running: isRunning(job),
    progress: job?.progress ?? 0,
    message: job?.message ?? null,
    stale: clips.filter(outdated).length,
    voiced,
    start
  }
}
