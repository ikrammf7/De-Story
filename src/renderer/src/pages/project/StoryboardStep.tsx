import { useEffect, useMemo, useRef, useState } from 'react'
import {
  ArrowLeft,
  ArrowRight,
  ChevronLeft,
  ChevronRight,
  Clapperboard,
  Film,
  Image as ImageIcon,
  MoreHorizontal,
  Move,
  Pause,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Sparkles,
  Trash2,
  Volume2
} from 'lucide-react'
import { effectiveResolution, fitDuration, getImageModel, getVideoModel } from '@shared/higgsfield'
import { CAMERA_PRESETS, MOTION_STRENGTHS, cameraAt, cameraCss } from '@shared/motion'
import { needsVisual } from '@shared/script'
import { geminiVoiceName, geminiVoiceTone } from '@shared/models'
import type { Asset, Clip } from '@shared/types'
import { AutoTextarea } from '../../components/AutoTextarea'
import { CharacterAvatar } from '../../components/CharacterAvatar'
import { ClipVisual } from '../../components/ClipVisual'
import { Badge, Button, Chip, IconButton, Menu, Progress, Segmented, confirmDialog, cx, inputCls } from '../../components/ui'
import { clipNumber, errorText, mmss, secLabel } from '../../lib/format'
import { useNarration } from '../../lib/useNarration'
import { useApp } from '../../store/app'
import { isRunning, jobFor, useProject } from '../../store/project'
import { CharacterModal } from './CharacterModal'

function useAudio(): { play: (url: string) => void; stop: () => void; playing: string | null } {
  const ref = useRef<HTMLAudioElement | null>(null)
  const [playing, setPlaying] = useState<string | null>(null)
  useEffect(() => () => ref.current?.pause(), [])
  return {
    playing,
    play: (url) => {
      ref.current?.pause()
      const a = new Audio(url)
      a.onended = () => setPlaying(null)
      ref.current = a
      void a.play()
      setPlaying(url)
    },
    stop: () => {
      ref.current?.pause()
      setPlaying(null)
    }
  }
}

/** Loops the camera move over the clip image, so a picked move is visible right away without rendering. */
function MotionPreview({ image, clip }: { image: Asset; clip: Clip }) {
  const img = useRef<HTMLImageElement>(null)
  const [playing, setPlaying] = useState(() => !window.matchMedia('(prefers-reduced-motion: reduce)').matches)
  const mounted = useRef(false)

  // Picking another move or strength starts the preview again, even after a pause.
  useEffect(() => {
    if (mounted.current) setPlaying(true)
    mounted.current = true
  }, [clip.cameraPreset, clip.motionStrength])

  useEffect(() => {
    const el = img.current
    if (!el) return
    if (!playing) {
      el.style.transform = ''
      return
    }
    const dur = Math.max(1, clip.durationMs / 1000)
    const t0 = performance.now()
    let raf = 0
    const tick = (): void => {
      // One pass of the move, a short hold on its last frame, then again.
      const t = Math.min(((performance.now() - t0) / 1000) % (dur + 0.6), dur)
      el.style.transform = cameraCss(cameraAt(clip.cameraPreset, clip.motionStrength, t / dur, t))
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [playing, clip.cameraPreset, clip.motionStrength, clip.durationMs, image.id])

  return (
    <>
      <img ref={img} src={image.url} alt="" draggable={false} className="absolute inset-0 size-full object-cover will-change-transform" />
      <button
        type="button"
        aria-pressed={playing}
        onClick={() => setPlaying(!playing)}
        className="absolute right-2.5 top-2.5 inline-flex h-8 items-center gap-1.5 rounded-full bg-ink/80 px-3 text-xs font-medium text-paper hover:bg-ink"
      >
        {playing ? <Pause className="size-3.5" /> : <Play className="size-3.5" />}
        {playing ? 'Jeda pratinjau' : 'Putar pratinjau'}
      </button>
    </>
  )
}

function ClipPanel({ clip, index, total }: { clip: Clip; index: number; total: number }) {
  const { project, clips, characters, assets, jobs, updateClip, updateProject, removeClip, moveClip, addClip, select, flush } = useProject()
  const { toast } = useApp()
  const audio = useAudio()
  const [editPrompt, setEditPrompt] = useState(false)
  if (!project) return null

  const image = clip.imageAssetId ? assets[clip.imageAssetId] : null
  const video = clip.videoAssetId ? assets[clip.videoAssetId] : null
  const voice = clip.audioAssetId ? assets[clip.audioAssetId] : null
  const imageJob = jobFor(jobs, 'image', { clipId: clip.id })
  const narration = useNarration()
  const videoJob = jobFor(jobs, 'video', { clipId: clip.id })
  const versions = Object.values(assets)
    .filter((a) => a.clipId === clip.id && a.kind === 'image')
    .sort((a, b) => a.createdAt - b.createdAt)
  const start = clips.slice(0, index).reduce((n, c) => n + c.durationMs, 0)
  const narrationChanged = !!voice && (voice.prompt ?? '').trim() !== clip.narration.trim()
  const voiceName =
    project.ttsProvider === 'gemini'
      ? `${geminiVoiceName(project.ttsVoice)} · ${geminiVoiceTone(project.ttsVoice) ?? 'Gemini'}`
      : 'ElevenLabs'

  const run = async (fn: () => Promise<unknown>): Promise<void> => {
    try {
      await flush()
      await fn()
    } catch (e) {
      toast('error', errorText(e))
    }
  }

  const remove = async (): Promise<void> => {
    const ok = await confirmDialog({
      title: `Hapus klip ${clipNumber(index)}?`,
      body: 'Klip beserta tautan gambar, video, dan suaranya akan dilepas dari storyboard.',
      confirm: 'Hapus klip',
      danger: true
    })
    if (ok) removeClip(clip.id)
  }

  // Capped by window height, so the settings under the fixed preview keep enough room.
  const aspect = project.aspectRatio === '9:16' ? 'aspect-[9/16] h-[min(300px,40vh)] mx-auto' : 'aspect-video h-[min(282px,36vh)] max-w-full mx-auto'

  return (
    <aside className="flex min-h-0 flex-col border-l border-line bg-surface">
      <div className="flex h-16 shrink-0 items-center gap-2.5 border-b border-line pl-5 pr-4">
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="font-mono text-xs text-muted">
            Klip {clipNumber(index)} · {mmss(start)} – {mmss(start + clip.durationMs)}
          </span>
          <input
            value={clip.title}
            onChange={(e) => updateClip(clip.id, { title: e.target.value })}
            aria-label="Judul klip"
            className="-ml-1 rounded-md bg-transparent px-1 font-display text-[19px] font-bold outline-none hover:bg-sand focus:bg-sand"
          />
        </div>
        <IconButton label="Klip sebelumnya" size={38} disabled={index === 0} onClick={() => select(clips[index - 1].id)}>
          <ChevronLeft className="size-4" />
        </IconButton>
        <IconButton label="Klip berikutnya" size={38} disabled={index === total - 1} onClick={() => select(clips[index + 1].id)}>
          <ChevronRight className="size-4" />
        </IconButton>
        <Menu
          trigger={(open) => (
            <IconButton label="Opsi klip" size={38} onClick={open}>
              <MoreHorizontal className="size-4" />
            </IconButton>
          )}
          items={[
            { label: 'Geser ke kiri', icon: <ArrowLeft className="size-4" />, run: () => moveClip(clip.id, -1) },
            { label: 'Geser ke kanan', icon: <ArrowRight className="size-4" />, run: () => moveClip(clip.id, 1) },
            { label: 'Tambah klip setelah ini', icon: <Plus className="size-4" />, run: () => addClip(clip.id) },
            { label: 'Hapus klip', icon: <Trash2 className="size-4" />, danger: true, run: () => void remove() }
          ]}
        />
      </div>

      {/* The preview stays put while the settings below it scroll. */}
      <div className="shrink-0 border-b border-line px-5 pb-3.5 pt-4">
        <div className={cx('relative overflow-hidden rounded-xl border-[1.5px] border-ink bg-paper', aspect)}>
          {image && !isRunning(imageJob) && clip.motionType === 'camera' ? (
            <MotionPreview image={image} clip={clip} />
          ) : (
            <ClipVisual clip={clip} image={image} video={video} job={imageJob} large playVideo />
          )}
          {!image && !isRunning(imageJob) && (
            <div className="absolute inset-x-0 bottom-6 flex justify-center">
              <Button variant="accent" icon={<Sparkles className="size-4" />} onClick={() => void run(() => window.api.generate.clipImage(clip.id))}>
                Generate gambar
              </Button>
            </div>
          )}
          {versions.length > 1 && (
            <div className="absolute bottom-2.5 left-2.5 flex items-center gap-1.5 rounded-[10px] border border-ink bg-surface/95 p-1.5">
              {versions.slice(-5).map((v, i, arr) => (
                <button
                  key={v.id}
                  type="button"
                  aria-label={`Pakai versi ${versions.length - arr.length + i + 1}`}
                  onClick={() => void window.api.assets.setActive(clip.id, v.id)}
                  className={cx(
                    'h-[26px] w-[44px] overflow-hidden rounded-[5px]',
                    v.id === clip.imageAssetId ? 'border-2 border-accent' : 'border border-line-2 opacity-80 hover:opacity-100'
                  )}
                >
                  <img src={v.url} alt="" className="size-full object-cover" />
                </button>
              ))}
              <span className="px-1 text-xs font-semibold">
                Versi {versions.findIndex((v) => v.id === clip.imageAssetId) + 1}
              </span>
            </div>
          )}
        </div>
      </div>

      <div className="scroll-thin flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-5 py-4">
        <div className="flex flex-wrap gap-2">
          {image && (
            <Button
              size="sm"
              icon={<RefreshCw className="size-4" />}
              loading={isRunning(imageJob)}
              onClick={() => void run(() => window.api.generate.clipImage(clip.id))}
            >
              Generate ulang
            </Button>
          )}
          <Button size="sm" icon={<Pencil className="size-4" />} onClick={() => setEditPrompt(!editPrompt)}>
            {editPrompt ? 'Tutup prompt' : 'Edit prompt visual'}
          </Button>
        </div>

        {editPrompt && (
          <AutoTextarea
            minRows={5}
            autoFocus
            value={clip.visualPrompt}
            onChange={(e) => updateClip(clip.id, { visualPrompt: e.target.value })}
            aria-label="Prompt visual"
            placeholder={[
              'Setting: tempat, dunia/planet, era, detail latar',
              'Action: siapa melakukan apa',
              'Wardrobe: Leo - default outfit as on the reference sheet; the teenager - custom for this scene: grey tunic',
              'Framing: medium shot, eye level',
              'Avoid: Earth landscape, aliens'
            ].join('\n')}
            className={cx(inputCls, 'py-2.5 text-[13px] leading-relaxed')}
          />
        )}

        {characters.length > 0 && (
          <div className="flex flex-col gap-1.5">
            <span className="text-sm font-semibold">Pemeran di klip ini</span>
            <div className="flex flex-wrap gap-1.5">
              {characters.map((c) => {
                const on = clip.characterIds.includes(c.id)
                const sheet = c.sheetAssetId ? assets[c.sheetAssetId] : null
                return (
                  <button
                    key={c.id}
                    type="button"
                    aria-pressed={on}
                    title={sheet ? undefined : 'Belum ada lembar karakter'}
                    onClick={() =>
                      updateClip(clip.id, {
                        characterIds: on ? clip.characterIds.filter((x) => x !== c.id) : [...clip.characterIds, c.id]
                      })
                    }
                    className={cx(
                      'inline-flex h-9 items-center gap-2 whitespace-nowrap rounded-full border py-0 pl-1 pr-3 text-[13px] font-medium transition-colors',
                      on ? 'border-ink bg-ink text-paper' : 'border-line-2 bg-surface text-ink hover:border-ink-2'
                    )}
                  >
                    <CharacterAvatar name={c.name} sheetUrl={sheet?.url} size={28} className={on ? 'border-paper/70' : undefined} />
                    {c.name}
                  </button>
                )
              })}
            </div>
          </div>
        )}

        <div className="flex flex-col gap-1.5">
          <div className="flex items-center gap-2">
            <label htmlFor="naskah" className="text-sm font-semibold">
              Naskah narasi
            </label>
            <span className="text-xs text-muted">untuk TTS</span>
            <span className="flex-1" />
            <span className="text-xs text-ink-2">{voiceName}</span>
            <span className="rounded-md bg-sand px-1.5 py-0.5 font-mono text-xs text-ink-2">
              {voice?.durationMs ? secLabel(voice.durationMs) : 'Belum ada suara'}
            </span>
          </div>
          <AutoTextarea
            id="naskah"
            minRows={3}
            value={clip.narration}
            onChange={(e) => updateClip(clip.id, { narration: e.target.value })}
            className={cx(inputCls, 'py-2.5 text-sm leading-relaxed')}
          />
          <div className="flex items-center gap-2">
            {voice && (
              <Button
                size="sm"
                icon={audio.playing === voice.url ? <Pause className="size-3.5" /> : <Play className="size-3.5" />}
                onClick={() => (audio.playing === voice.url ? audio.stop() : audio.play(voice.url))}
              >
                {audio.playing === voice.url ? 'Hentikan' : 'Putar suara'}
              </Button>
            )}
            {/* Voices come from one take of the whole script (toolbar button), never per scene. */}
            {narration.running ? (
              <span className="text-xs text-sun-ink">Suara narasi sedang dibuat…</span>
            ) : narrationChanged ? (
              <span className="text-xs text-accent-dark">
                Naskah berubah.{' '}
                <button type="button" onClick={() => void narration.start()} className="font-semibold underline">
                  Rekam ulang suara narasi
                </button>
              </span>
            ) : (
              !voice && <span className="text-xs text-muted">Suara semua adegan dibuat sekaligus lewat tombol Buat suara narasi di atas.</span>
            )}
          </div>
        </div>

        <div className="flex flex-col gap-2 pb-2">
          <span className="text-sm font-semibold">Gerak</span>
          <div className="grid grid-cols-2 gap-2">
            {(
              [
                { id: 'camera', title: 'Gerak kamera', sub: 'Gratis · dirender di komputer', icon: <Move className="size-[22px]" />, subCls: 'text-ok-ink' },
                { id: 'video', title: 'Video AI', sub: 'Higgsfield · pakai kredit', icon: <Film className="size-[22px]" />, subCls: 'text-ink-2' }
              ] as const
            ).map((o) => {
              const on = clip.motionType === o.id
              return (
                <button
                  key={o.id}
                  type="button"
                  aria-pressed={on}
                  onClick={() => updateClip(clip.id, { motionType: o.id })}
                  className={cx(
                    'flex items-center gap-2.5 rounded-xl px-3 py-2 text-left',
                    on ? 'border-2 border-accent bg-accent-soft' : 'border border-line-2 bg-surface hover:border-ink-2'
                  )}
                >
                  {o.icon}
                  <span className="flex flex-col">
                    <span className="text-sm font-semibold">{o.title}</span>
                    <span className={cx('text-xs', o.subCls)}>{o.sub}</span>
                  </span>
                </button>
              )
            })}
          </div>
          {clip.motionType === 'camera' ? (
            <>
              <div className="flex flex-wrap gap-1.5">
                {CAMERA_PRESETS.map((p) => (
                  <Chip key={p.id} on={clip.cameraPreset === p.id} className="h-8 px-3 text-[13px]" onClick={() => updateClip(clip.id, { cameraPreset: p.id })}>
                    {p.label}
                  </Chip>
                ))}
              </div>
              <Segmented
                size="sm"
                value={clip.motionStrength}
                onChange={(v) => updateClip(clip.id, { motionStrength: v })}
                options={MOTION_STRENGTHS.map((m) => ({ id: m.id, label: m.label }))}
                className="self-start"
              />
            </>
          ) : (
            <div className="flex flex-col gap-2">
              {!voice && (
                <p className="rounded-lg bg-sun-soft px-2.5 py-1.5 text-xs text-sun-ink">
                  Buat suara narasinya dulu. Panjang klip mengikuti suara, jadi durasi video AI ikut pas.
                </p>
              )}
              {video?.durationMs != null && video.durationMs + 300 < clip.durationMs && (
                <p className="rounded-lg bg-sun-soft px-2.5 py-1.5 text-xs text-sun-ink">
                  Video {secLabel(video.durationMs)}, klip {secLabel(clip.durationMs)}. Sisa {secLabel(clip.durationMs - video.durationMs)} memakai frame
                  terakhir yang diam. Buat ulang video supaya pas.
                </p>
              )}
              {project && (
                <div className="flex items-center gap-1.5 text-xs text-ink-2">
                  <Film className="size-3.5 shrink-0" />
                  <span className="truncate">
                    {getVideoModel(project.videoModel).name} · video {fitDuration(getVideoModel(project.videoModel), clip.durationMs / 1000)} detik
                    {effectiveResolution(getVideoModel(project.videoModel), project.videoResolution) &&
                      ` · ${effectiveResolution(getVideoModel(project.videoModel), project.videoResolution)}`}
                  </span>
                  <button
                    type="button"
                    onClick={() => updateProject({ step: 1 })}
                    className="shrink-0 font-semibold text-accent-dark hover:underline"
                  >
                    Ganti model
                  </button>
                </div>
              )}
              <AutoTextarea
                minRows={2}
                value={clip.videoPrompt}
                onChange={(e) => updateClip(clip.id, { videoPrompt: e.target.value })}
                placeholder="Arahan gerak, misalnya: Gajah Mada mengangkat kepalan tangan, kamera mendekat pelan"
                aria-label="Arahan gerak video"
                className={cx(inputCls, 'py-2 text-[13px] leading-relaxed')}
              />
              <div className="flex items-center gap-2">
                <Button
                  size="sm"
                  variant={video ? 'secondary' : 'accent'}
                  icon={<Clapperboard className="size-4" />}
                  loading={isRunning(videoJob)}
                  disabled={!image}
                  onClick={() => void run(() => window.api.generate.clipVideo(clip.id))}
                >
                  {video ? 'Buat ulang video' : 'Buat video'}
                </Button>
                {!image && <span className="text-xs text-muted">Buat gambar dulu, lalu jadikan video.</span>}
                {isRunning(videoJob) && (
                  <span className="text-xs text-sun-ink">
                    {videoJob!.message ?? 'Memproses'} {Math.round(videoJob!.progress * 100)}%
                  </span>
                )}
                {videoJob?.status === 'failed' && <span className="truncate text-xs text-bad-ink">{videoJob.error}</span>}
              </div>
            </div>
          )}
        </div>
      </div>
    </aside>
  )
}

export function StoryboardStep() {
  const { project, clips, characters, assets, jobs, selectedClipId, select, addClip, updateProject, flush, load } = useProject()
  const { toast } = useApp()
  /** A character id, "new" for an unsaved one, or null. */
  const [openChar, setOpenChar] = useState<string | null>(null)
  const [credits, setCredits] = useState<number | null>(null)
  const [videoCredits, setVideoCredits] = useState<number | null>(null)
  const [planning, setPlanning] = useState(false)
  const lastNarration = useRef<string | null>(null)

  // A failed narration take is reported once, when it happens.
  useEffect(() => {
    const j = jobFor(jobs, 'narration', {})
    const key = j ? `${j.id}:${j.status}` : null
    if (j?.status === 'failed' && lastNarration.current && lastNarration.current !== key) toast('error', j.error ?? 'Gagal membuat suara narasi')
    lastNarration.current = key
  }, [jobs])

  useEffect(() => {
    if (project)
      void window.api.generate
        .estimate('image', getImageModel(project.imageModel).id, project.aspectRatio)
        .then((e) => setCredits(e?.credits ?? null))
  }, [project?.id, project?.imageModel, project?.aspectRatio])

  const stats = useMemo(() => {
    const imgs = clips.filter((c) => c.imageAssetId).length
    const voices = clips.filter((c) => c.audioAssetId).length
    const needImg = clips.filter((c) => !c.imageAssetId && !isRunning(jobFor(jobs, 'image', { clipId: c.id }))).length
    const videoRunning = clips.filter((c) => isRunning(jobFor(jobs, 'video', { clipId: c.id }))).length
    const needVideo = clips.filter((c) => !c.videoAssetId && !isRunning(jobFor(jobs, 'video', { clipId: c.id })))
    return { imgs, voices, needImg, videoRunning, needVideo, total: clips.reduce((n, c) => n + c.durationMs, 0) }
  }, [clips, jobs])

  // Video length follows each clip's voice, so the cost is estimated per video length and summed.
  const videoLengths = useMemo(() => {
    if (!project) return ''
    const model = getVideoModel(project.videoModel)
    return stats.needVideo.map((c) => fitDuration(model, c.durationMs / 1000) ?? 5).join(',')
  }, [stats.needVideo, project?.videoModel])
  useEffect(() => {
    if (!project || !videoLengths) return setVideoCredits(null)
    const lengths = videoLengths.split(',').map(Number)
    const unique = [...new Set(lengths)]
    const model = getVideoModel(project.videoModel).id
    let live = true
    void Promise.all(unique.map((s) => window.api.generate.estimate('video', model, project.aspectRatio, s, project.videoResolution))).then((list) => {
      if (!live) return
      if (list.some((e) => e?.credits == null)) return setVideoCredits(null)
      const bySec = new Map(unique.map((s, i) => [s, list[i]!.credits]))
      setVideoCredits(lengths.reduce((n, s) => n + (bySec.get(s) ?? 0), 0))
    })
    return () => {
      live = false
    }
  }, [videoLengths, project?.aspectRatio, project?.videoResolution])

  if (!project) return null
  const selIndex = Math.max(0, clips.findIndex((c) => c.id === selectedClipId))
  const sel = clips[selIndex]

  const bulk = async (fn: () => Promise<unknown>): Promise<void> => {
    try {
      await flush()
      await fn()
    } catch (e) {
      toast('error', errorText(e))
    }
  }

  const narration = useNarration()
  // Videos are made last: every clip has its picture, and the voice is final so each video matches its clip length.
  const readyForVideo =
    clips.length > 0 &&
    clips.every((c) => c.imageAssetId) &&
    narration.voiced > 0 &&
    narration.stale === 0 &&
    !narration.running
  const makeVideos = async (): Promise<void> => {
    const n = stats.needVideo.length
    const ok = await confirmDialog({
      title: `Jadikan ${n} klip video AI?`,
      body:
        `Setiap gambar klip dianimasikan dengan ${getVideoModel(project!.videoModel).name}, panjangnya mengikuti suara narasi klip itu.` +
        (videoCredits != null ? ` Perkiraan biaya ${Math.round(videoCredits * 10) / 10} kredit Higgsfield.` : '') +
        ' Klip yang sudah punya video tidak dibuat ulang.',
      confirm: `Buat ${n} video`
    })
    if (ok) await bulk(() => window.api.generate.missingVideos(project!.id))
  }

  // Scenes added or rewritten in the script since the last visual plan.
  const stale = clips.filter(needsVisual).length
  const planVisuals = async (): Promise<void> => {
    setPlanning(true)
    try {
      await flush()
      load(await window.api.story.visuals(project.id, 'missing'))
      toast('success', 'Visual adegan sudah disusun.')
    } catch (e) {
      toast('error', errorText(e))
    } finally {
      setPlanning(false)
    }
  }

  if (!clips.length)
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 text-center">
        <h2 className="font-display text-3xl font-bold">Storyboard masih kosong</h2>
        <p className="max-w-md text-ink-2">Tulis naskahnya dulu di langkah Naskah, lalu buat visual tiap adegan. Kamu juga bisa menambah klip sendiri.</p>
        <div className="flex gap-2.5">
          <Button onClick={() => updateProject({ step: 2 })}>Ke Naskah</Button>
          <Button variant="primary" icon={<Plus className="size-4" />} onClick={() => addClip(null)}>
            Tambah klip
          </Button>
        </div>
      </div>
    )

  return (
    <div className="grid h-full grid-rows-[64px_minmax(0,1fr)_80px]">
      <div className="flex items-center gap-2 border-b border-line bg-paper px-6">
        <span className="mr-1 text-[13px] font-semibold text-ink-2">Pemeran</span>
        <div className="scroll-thin flex min-w-0 gap-2 overflow-x-auto py-1">
          {characters.map((c) => {
            const sheet = c.sheetAssetId ? assets[c.sheetAssetId] : null
            const running = isRunning(jobFor(jobs, 'sheet', { characterId: c.id }))
            return (
              <button
                key={c.id}
                type="button"
                onClick={() => setOpenChar(c.id)}
                className="inline-flex h-10 shrink-0 items-center gap-2 rounded-full border border-line-2 bg-surface py-0 pl-1 pr-3 text-[13px] font-medium hover:border-ink-2"
              >
                <CharacterAvatar name={c.name} sheetUrl={sheet?.url} size={32} busy={running} />
                {c.name}
                {!sheet && !running && <span className="size-1.5 rounded-full bg-accent" title="Belum ada lembar karakter" />}
              </button>
            )
          })}
          <button
            type="button"
            onClick={() => setOpenChar('new')}
            className="inline-flex h-10 shrink-0 items-center gap-1.5 rounded-full border border-dashed border-line-3 px-3.5 text-[13px] font-medium text-ink-2 hover:border-ink-2"
          >
            <Plus className="size-[15px]" />
            Tambah pemeran
          </button>
        </div>
        <div className="flex-1" />
        <Button
          icon={<Volume2 className="size-[17px]" />}
          loading={narration.running}
          disabled={narration.running || !clips.some((c) => c.narration.trim())}
          title="Narasi semua adegan direkam dalam satu kali jalan lalu dipotong per adegan, supaya suara dan intonasinya konsisten."
          onClick={() => void narration.start()}
        >
          {narration.running
            ? `${narration.message ?? 'Membuat suara'} ${Math.round(narration.progress * 100)}%`
            : narration.voiced === 0
              ? 'Buat suara narasi'
              : narration.stale
                ? `Perbarui suara narasi (${narration.stale})`
                : 'Rekam ulang suara narasi'}
        </Button>
        <Button
          className="border-[1.5px] border-ink font-semibold"
          icon={<ImageIcon className="size-[17px]" />}
          disabled={!stats.needImg}
          onClick={() => void bulk(() => window.api.generate.missingImages(project.id))}
        >
          Generate {stats.needImg} gambar
          {credits != null && stats.needImg > 0 && (
            <span className="font-normal text-ink-2">≈ {Math.round(credits * stats.needImg * 10) / 10} kr</span>
          )}
        </Button>
        {readyForVideo && (
          <Button
            variant="primary"
            icon={<Clapperboard className="size-[17px]" />}
            loading={stats.videoRunning > 0}
            disabled={stats.videoRunning > 0 || stats.needVideo.length === 0}
            title="Semua gambar dan suara narasi sudah siap. Setiap klip dijadikan video AI sepanjang suaranya."
            onClick={() => void makeVideos()}
          >
            {stats.videoRunning > 0
              ? `Membuat ${stats.videoRunning} video`
              : stats.needVideo.length === 0
                ? 'Semua klip sudah video'
                : `Jadikan ${stats.needVideo.length} video`}
            {stats.videoRunning === 0 && videoCredits != null && stats.needVideo.length > 0 && (
              <span className="font-normal text-white/80">≈ {Math.round(videoCredits * 10) / 10} kr</span>
            )}
          </Button>
        )}
      </div>

      <div className="grid min-h-0 grid-cols-[minmax(0,1fr)_540px]">
        <section className="scroll-thin flex min-h-0 flex-col gap-3.5 overflow-y-auto px-6 pb-6 pt-[18px]">
          <div className="flex items-center gap-3">
            <h1 className="font-display text-2xl font-bold">Storyboard</h1>
            <span className="text-sm text-muted">
              {clips.length} klip · {mmss(stats.total)}
            </span>
            <div className="flex-1" />
            <Button variant="ghost" icon={<Plus className="size-[17px]" />} onClick={() => addClip(sel?.id ?? null)}>
              Tambah klip
            </Button>
          </div>
          {stale > 0 && (
            <div className="flex items-center gap-3 rounded-2xl border border-sun bg-sun-soft px-4 py-3 text-sm text-sun-ink">
              <Sparkles className="size-[18px] shrink-0" />
              <span className="flex-1">
                {stale} adegan belum punya visual dari naskah terbaru. Buat visualnya dulu supaya gambar sesuai dengan alur cerita.
              </span>
              <Button size="sm" variant="secondary" loading={planning} onClick={() => void planVisuals()}>
                Buat visual
              </Button>
            </div>
          )}
          <div className={cx('grid gap-3.5', project.aspectRatio === '9:16' ? 'grid-cols-4' : 'grid-cols-3')}>
            {clips.map((c, i) => {
              const on = c.id === sel?.id
              const imageJob = jobFor(jobs, 'image', { clipId: c.id })
              const ttsRunning = isRunning(jobFor(jobs, 'tts', { clipId: c.id }))
              const status = c.imageAssetId ? { label: 'Siap', tone: 'ok' as const } : isRunning(imageJob) ? { label: 'Proses', tone: 'warn' as const } : imageJob?.status === 'failed' ? { label: 'Gagal', tone: 'bad' as const } : { label: 'Draf', tone: 'draft' as const }
              return (
                <button
                  key={c.id}
                  type="button"
                  aria-pressed={on}
                  onClick={() => select(c.id)}
                  className={cx(
                    'flex flex-col overflow-hidden rounded-[14px] bg-surface text-left',
                    on ? 'border-2 border-accent shadow-ink-md' : 'border border-line hover:border-line-3'
                  )}
                >
                  <div className={cx('relative w-full overflow-hidden bg-paper', project.aspectRatio === '9:16' ? 'aspect-[9/16]' : 'aspect-video')}>
                    <ClipVisual
                      clip={c}
                      image={c.imageAssetId ? assets[c.imageAssetId] : null}
                      video={c.videoAssetId ? assets[c.videoAssetId] : null}
                      job={imageJob}
                    />
                    <span className="absolute left-2 top-2 rounded-md bg-ink px-[7px] py-0.5 font-mono text-xs text-paper">{clipNumber(i)}</span>
                    {c.motionType === 'video' && (
                      <span className="absolute right-2 top-2 flex size-6 items-center justify-center rounded-md bg-ink text-paper" title="Video AI">
                        <Film className="size-3.5" />
                      </span>
                    )}
                    <span className="absolute bottom-2 right-2 rounded-md border border-ink bg-surface px-[7px] py-px font-mono text-xs">
                      {mmss(c.durationMs)}
                    </span>
                  </div>
                  <div className="flex w-full flex-col gap-2 px-3 pb-3 pt-2.5">
                    <span className="truncate text-sm font-semibold">{c.title || 'Tanpa judul'}</span>
                    <div className="flex items-center gap-2">
                      <Badge tone={status.tone}>{status.label}</Badge>
                      <span className={cx('inline-flex items-center gap-1 text-xs', c.audioAssetId ? 'text-ok-ink' : 'text-muted')}>
                        <Volume2 className="size-3.5" />
                        {ttsRunning ? 'Membuat suara…' : c.audioAssetId ? 'Suara siap' : 'Belum ada suara'}
                      </span>
                    </div>
                  </div>
                </button>
              )
            })}
          </div>
        </section>
        {sel && <ClipPanel key={sel.id} clip={sel} index={selIndex} total={clips.length} />}
      </div>

      <footer className="flex items-center gap-4 border-t border-line bg-surface px-6">
        <div className="flex w-[320px] flex-col gap-1.5">
          <div className="flex justify-between text-[13px]">
            <span className="font-semibold">
              {stats.imgs} dari {clips.length} gambar siap
            </span>
            <span className="text-muted">
              {stats.voices} dari {clips.length} suara siap
            </span>
          </div>
          <Progress value={clips.length ? (stats.imgs + stats.voices) / (clips.length * 2) : 0} />
        </div>
        <span className="text-[13px] text-muted">Klip yang belum siap tetap bisa dibuka di editor.</span>
        <div className="flex-1" />
        <Button size="lg" onClick={() => updateProject({ step: 2 })}>
          Kembali ke naskah
        </Button>
        <Button
          variant="primary"
          size="lg"
          className="h-[50px]"
          onClick={() => updateProject({ step: 4, status: project.status === 'exported' ? 'exported' : 'editing' })}
        >
          Lanjut ke editor
          <ArrowRight className="size-[18px]" />
        </Button>
      </footer>
      <CharacterModal characterId={openChar} onClose={() => setOpenChar(null)} onCreated={(id) => setOpenChar(id)} />
    </div>
  )
}
