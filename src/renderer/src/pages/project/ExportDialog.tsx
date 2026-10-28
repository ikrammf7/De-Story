import { useEffect, useState } from 'react'
import { AlertTriangle, CheckCircle2, Download, FolderOpen } from 'lucide-react'
import type { ExportOptions, Job } from '@shared/types'
import { Button, Modal, Progress, Switch, cx, inputCls } from '../../components/ui'
import { clipNumber, errorText, mmss } from '../../lib/format'
import { useApp } from '../../store/app'
import { isRunning, useProject } from '../../store/project'

const RESOLUTIONS: { id: ExportOptions['resolution']; label: string; sub: string; mb: number }[] = [
  { id: 720, label: '720p', sub: 'Lebih kecil', mb: 0.4 },
  { id: 1080, label: '1080p', sub: 'Disarankan', mb: 0.75 },
  { id: 2160, label: '4K', sub: 'Lebih lama', mb: 2.4 }
]

function slug(s: string): string {
  return (
    s
      .toLowerCase()
      .normalize('NFKD')
      .replace(/[^\p{L}\p{N}]+/gu, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 60) || 'video'
  )
}

export function ExportDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { project, clips, jobs, flush } = useProject()
  const { toast } = useApp()
  const [opts, setOpts] = useState<ExportOptions | null>(null)
  const [jobId, setJobId] = useState<string | null>(null)

  useEffect(() => {
    if (!open || !project) return
    void window.api.exporter.defaultFolder().then((folder) =>
      setOpts({
        fileName: slug(project.title),
        resolution: 1080,
        fps: 30,
        burnCaptions: project.editor.captionStyle !== 'none',
        writeSrt: false,
        normalizeAudio: true,
        duckMusic: project.editor.duckMusic,
        folder
      })
    )
  }, [open])

  if (!project || !opts) return null
  const job: Job | null = jobId ? (jobs[jobId] ?? null) : null
  const running = isRunning(job)
  const totalMs = clips.reduce((n, c) => n + c.durationMs, 0)
  const missing = clips
    .map((c, i) => ({ c, i }))
    .filter(({ c }) => !(c.imageAssetId || (c.motionType === 'video' && c.videoAssetId)))
  const noVoice = clips.filter((c) => !c.audioAssetId).length
  const res = RESOLUTIONS.find((r) => r.id === opts.resolution)!
  const sizeMb = Math.round((totalMs / 1000) * res.mb * (opts.fps === 60 ? 1.5 : 1))
  const set = (p: Partial<ExportOptions>): void => setOpts({ ...opts, ...p })

  const start = async (): Promise<void> => {
    try {
      await flush()
      const j = await window.api.exporter.start(project.id, opts)
      useProject.getState().upsertJob(j)
      setJobId(j.id)
    } catch (e) {
      toast('error', errorText(e))
    }
  }

  const toggles: { key: keyof ExportOptions; title: string; desc: string; disabled?: boolean }[] = [
    {
      key: 'burnCaptions',
      title: 'Tempel caption ke video',
      desc: project.editor.captionStyle === 'none' ? 'Gaya caption diatur ke Tanpa caption' : 'Caption jadi bagian dari gambar',
      disabled: project.editor.captionStyle === 'none'
    },
    { key: 'writeSrt', title: 'Simpan juga file subtitle (.srt)', desc: 'Untuk diunggah terpisah ke YouTube' },
    { key: 'normalizeAudio', title: 'Samakan volume suara', desc: 'Normalisasi ke −14 LUFS, standar YouTube' },
    {
      key: 'duckMusic',
      title: 'Kecilkan musik saat narasi',
      desc: project.editor.musicAssetId ? 'Musik otomatis turun ketika ada suara' : 'Belum ada musik latar',
      disabled: !project.editor.musicAssetId
    }
  ]

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Ekspor video"
      subtitle={`${project.title} · ${mmss(totalMs)} · ${clips.length} klip · ${project.aspectRatio}`}
      width={640}
    >
      {job?.status === 'done' ? (
        <div className="flex flex-col items-center gap-3 py-4 text-center">
          <CheckCircle2 className="size-12 text-ok-ink" />
          <p className="text-lg font-semibold">Video selesai diekspor</p>
          <p className="break-all font-mono text-[13px] text-ink-2">{job.message}</p>
          <div className="flex gap-2.5">
            <Button icon={<FolderOpen className="size-4" />} onClick={() => job.message && void window.api.exporter.reveal(job.message)}>
              Buka folder
            </Button>
            <Button variant="ink" onClick={onClose}>
              Selesai
            </Button>
          </div>
        </div>
      ) : (
        <>
          {missing.length > 0 && (
            <div className="flex items-start gap-2.5 rounded-xl border border-sun bg-sun-soft px-3.5 py-2.5 text-[13px] text-sun-ink">
              <AlertTriangle className="mt-0.5 size-4 shrink-0" />
              <span>
                Klip {missing.map(({ i }) => clipNumber(i)).join(', ')} belum punya gambar. Buat gambarnya dulu di Storyboard sebelum ekspor.
              </span>
            </div>
          )}
          {missing.length === 0 && noVoice > 0 && (
            <div className="flex items-start gap-2.5 rounded-xl border border-line bg-sand px-3.5 py-2.5 text-[13px] text-ink-2">
              <AlertTriangle className="mt-0.5 size-4 shrink-0" />
              <span>{noVoice} klip belum punya suara narasi. Klip itu akan tampil tanpa suara dan tanpa caption.</span>
            </div>
          )}
          <label className="flex flex-col gap-1.5">
            <span className="text-sm font-semibold">Nama file</span>
            <div className="flex h-[46px] overflow-hidden rounded-xl border border-line-2">
              <input value={opts.fileName} onChange={(e) => set({ fileName: e.target.value })} className="flex-1 px-3.5 text-[15px] outline-none" />
              <span className="flex items-center border-l border-line-2 bg-sand px-3.5 font-mono text-sm text-ink-2">.mp4</span>
            </div>
          </label>
          <div className="grid grid-cols-[minmax(0,1fr)_170px] gap-3.5">
            <div className="flex flex-col gap-1.5">
              <span className="text-sm font-semibold">Resolusi</span>
              <div className="grid grid-cols-3 gap-1.5">
                {RESOLUTIONS.map((r) => {
                  const on = r.id === opts.resolution
                  return (
                    <button
                      key={r.id}
                      type="button"
                      aria-pressed={on}
                      onClick={() => set({ resolution: r.id })}
                      className={cx('flex h-14 flex-col items-center justify-center rounded-xl', on ? 'border-2 border-accent bg-accent-soft' : 'border border-line-2 hover:border-ink-2')}
                    >
                      <span className="text-[15px] font-semibold">{r.label}</span>
                      <span className="text-xs text-ink-2">{r.sub}</span>
                    </button>
                  )
                })}
              </div>
            </div>
            <div className="flex flex-col gap-1.5">
              <span className="text-sm font-semibold">Frame rate</span>
              <div className="grid grid-cols-2 gap-1.5">
                {([30, 60] as const).map((f) => (
                  <button
                    key={f}
                    type="button"
                    aria-pressed={opts.fps === f}
                    onClick={() => set({ fps: f })}
                    className={cx(
                      'h-14 rounded-xl text-[15px] font-semibold',
                      opts.fps === f ? 'border-2 border-accent bg-accent-soft' : 'border border-line-2 hover:border-ink-2'
                    )}
                  >
                    {f} fps
                  </button>
                ))}
              </div>
            </div>
          </div>
          <div className="flex flex-col rounded-[14px] border border-line">
            {toggles.map((t, i) => (
              <div key={t.key} className={cx('flex items-center gap-3.5 px-3.5 py-2.5', i > 0 && 'border-t border-line', t.disabled && 'opacity-60')}>
                <div className="flex flex-1 flex-col">
                  <span className="text-sm font-semibold">{t.title}</span>
                  <span className="text-[13px] text-muted">{t.desc}</span>
                </div>
                <Switch on={!!opts[t.key] && !t.disabled} onChange={(v) => !t.disabled && set({ [t.key]: v })} label={t.title} />
              </div>
            ))}
          </div>
          <div className="flex flex-col gap-1.5">
            <span className="text-sm font-semibold">Simpan ke</span>
            <div className="flex gap-2">
              <div className={cx(inputCls, 'flex h-11 min-w-0 flex-1 items-center gap-2.5 bg-sand font-mono text-[13px]')}>
                <FolderOpen className="size-[17px] shrink-0 text-ink-2" />
                <span className="truncate">{opts.folder}</span>
              </div>
              <Button
                className="h-11"
                onClick={async () => {
                  const f = await window.api.exporter.pickFolder()
                  if (f) set({ folder: f })
                }}
              >
                Ganti folder
              </Button>
            </div>
          </div>
          <div className="grid grid-cols-3 gap-2 rounded-[14px] bg-sand px-3.5 py-3">
            <div className="flex flex-col">
              <span className="text-xs text-ink-2">Durasi</span>
              <span className="font-mono text-base font-semibold">{mmss(totalMs)}</span>
            </div>
            <div className="flex flex-col">
              <span className="text-xs text-ink-2">Perkiraan ukuran</span>
              <span className="text-base font-semibold">± {sizeMb} MB</span>
            </div>
            <div className="flex flex-col">
              <span className="text-xs text-ink-2">Biaya</span>
              <span className="text-base font-semibold text-ok-ink">0 kredit</span>
            </div>
          </div>
          {job && (running || job.status === 'failed') && (
            <div className="flex flex-col gap-1.5">
              <div className="flex justify-between text-[13px]">
                <span className={job.status === 'failed' ? 'text-bad-ink' : 'text-ink-2'}>
                  {job.status === 'failed' ? job.error : (job.message ?? 'Menyiapkan…')}
                </span>
                {running && <span className="font-mono">{Math.round(job.progress * 100)}%</span>}
              </div>
              {running && <Progress value={job.progress} />}
            </div>
          )}
          <div className="flex items-center gap-2.5">
            <span className="flex-1 text-[13px] text-muted">
              {running && 'Kamu boleh menutup dialog ini, ekspor tetap berjalan.'}
            </span>
            {running ? (
              <Button variant="danger" onClick={() => job && void window.api.generate.cancel(job.id)}>
                Batalkan
              </Button>
            ) : (
              <Button onClick={onClose}>Batal</Button>
            )}
            <Button variant="primary" size="lg" icon={<Download className="size-[18px]" />} loading={running} disabled={missing.length > 0} onClick={() => void start()}>
              {running ? 'Mengekspor…' : 'Mulai ekspor'}
            </Button>
          </div>
        </>
      )}
    </Modal>
  )
}
