import { useEffect, useState } from 'react'
import { AudioLines, Check, Clapperboard, Copy, ExternalLink, Film, Layers, Move, Music, Sparkles, Type, Volume2, X } from 'lucide-react'
import { CAPTION_FONTS, CAPTION_POSITIONS, CAPTION_STYLES, captionScaleOf, getCaptionFont, resolveCaption } from '@shared/captions'
import { CAMERA_PRESETS, MOTION_STRENGTHS } from '@shared/motion'
import type { Clip } from '@shared/types'
import { Select } from '../../../components/Select'
import { AutoTextarea } from '../../../components/AutoTextarea'
import { Button, Segmented, Switch, cx, inputCls } from '../../../components/ui'
import { clipNumber, errorText, secLabel } from '../../../lib/format'
import { useApp } from '../../../store/app'
import { useNarration } from '../../../lib/useNarration'
import { isRunning, jobFor, useProject } from '../../../store/project'
import { OverlayPanel } from './OverlayPanel'

export type InspectorTab = 'klip' | 'caption' | 'overlay' | 'musik'

export const INSPECTOR_TABS: { id: InspectorTab; label: string; icon: typeof Move }[] = [
  { id: 'klip', label: 'Klip', icon: Move },
  { id: 'caption', label: 'Caption', icon: Type },
  { id: 'overlay', label: 'Overlay', icon: Layers },
  { id: 'musik', label: 'Musik', icon: Music }
]

function Section({ title, children, className }: { title: string; children: React.ReactNode; className?: string }) {
  return (
    <div className={cx('flex flex-col gap-2.5 border-b border-line px-[18px] py-4', className)}>
      <span className="text-sm font-semibold">{title}</span>
      {children}
    </div>
  )
}

function ClipTab({ clip, index }: { clip: Clip; index: number }) {
  const { assets, jobs, updateClip, flush } = useProject()
  const { toast } = useApp()
  const image = clip.imageAssetId ? assets[clip.imageAssetId] : null
  const video = clip.videoAssetId ? assets[clip.videoAssetId] : null
  const voice = clip.audioAssetId ? assets[clip.audioAssetId] : null
  const videoJob = jobFor(jobs, 'video', { clipId: clip.id })
  const narration = useNarration()
  const run = async (fn: () => Promise<unknown>): Promise<void> => {
    try {
      await flush()
      await fn()
    } catch (e) {
      toast('error', errorText(e))
    }
  }

  return (
    <>
      <div className="flex h-14 items-center gap-2 border-b border-line px-[18px]">
        <span className="rounded-md bg-ink px-[7px] py-0.5 font-mono text-xs text-paper">{clipNumber(index)}</span>
        <span className="truncate text-[15px] font-semibold">{clip.title}</span>
        <span className="flex-1" />
        <span className="font-mono text-xs text-muted">{secLabel(clip.durationMs)}</span>
      </div>
      {clip.motionType === 'video' ? (
        <Section title="Video AI">
          {video ? (
            <p className="text-[13px] text-ink-2">Klip ini memakai video dari Higgsfield.</p>
          ) : (
            <p className="text-[13px] text-ink-2">Video belum dibuat. Sampai video siap, pratinjau memakai gambar diam.</p>
          )}
          <div className="flex flex-wrap gap-2">
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
            <Button size="sm" icon={<Move className="size-4" />} onClick={() => updateClip(clip.id, { motionType: 'camera' })}>
              Kembali ke gerak kamera
            </Button>
          </div>
          {isRunning(videoJob) && (
            <span className="text-xs text-sun-ink">
              {videoJob!.message ?? 'Memproses'} {Math.round(videoJob!.progress * 100)}%
            </span>
          )}
        </Section>
      ) : (
        <Section title="Animasi kamera">
          <div className="grid grid-cols-[minmax(0,1fr)_120px] gap-2">
            <Select
              label="Gerak kamera"
              size="sm"
              value={clip.cameraPreset}
              onChange={(v) => updateClip(clip.id, { cameraPreset: v })}
              options={CAMERA_PRESETS.map((p) => ({ value: p.id, label: p.label }))}
            />
            <Select
              label="Kekuatan gerak"
              size="sm"
              value={clip.motionStrength}
              onChange={(v) => updateClip(clip.id, { motionStrength: v })}
              options={MOTION_STRENGTHS.map((m) => ({ value: m.id, label: m.label }))}
            />
          </div>
          <Button
            size="sm"
            className="self-start"
            icon={<Film className="size-4" />}
            disabled={!image}
            onClick={() => {
              updateClip(clip.id, { motionType: 'video' })
              if (!video) void run(() => window.api.generate.clipVideo(clip.id))
            }}
          >
            Jadikan video AI
          </Button>
        </Section>
      )}
      <Section title="Suara narasi" className="border-b-0">
        <p className="text-[13px] leading-relaxed text-ink-2">{clip.narration || 'Naskah masih kosong.'}</p>
        {/* Voices come from one take of the whole script, so a change is fixed by re-recording that take. */}
        {narration.running ? (
          <span className="text-xs text-sun-ink">Suara narasi sedang dibuat…</span>
        ) : !voice || (voice.prompt ?? '').trim() !== clip.narration.trim() ? (
          <div className="flex flex-col items-start gap-2">
            <span className="text-xs text-ink-2">{voice ? 'Naskah berubah sejak suara dibuat.' : 'Klip ini belum punya suara.'}</span>
            <Button size="sm" variant="accent" icon={<Volume2 className="size-4" />} onClick={() => void narration.start()}>
              {narration.voiced ? 'Rekam ulang suara narasi' : 'Buat suara narasi'}
            </Button>
          </div>
        ) : (
          voice.meta.estimatedTimings && <span className="text-xs text-muted">Waktu caption masih perkiraan</span>
        )}
      </Section>
    </>
  )
}

/** Whether caption words follow the real voice, and a way to fix the ones that are still estimates. */
function CaptionTiming() {
  const { project, clips, assets, jobs, flush } = useProject()
  const { toast, go } = useApp()
  const [whisper, setWhisper] = useState<boolean | null>(null)
  useEffect(() => {
    void window.api.whisper.status().then((s) => setWhisper(s.state === 'ready'))
    return window.api.on.whisper((s) => setWhisper(s.state === 'ready'))
  }, [])
  if (!project) return null
  const voiced = clips.filter((c) => c.audioAssetId && assets[c.audioAssetId])
  const estimated = voiced.filter((c) => assets[c.audioAssetId!]?.meta.estimatedTimings).length
  const job = jobFor(jobs, 'align', {})
  const running = isRunning(job)

  const sync = async (): Promise<void> => {
    try {
      await flush()
      await window.api.generate.syncCaptions(project.id)
    } catch (e) {
      toast('error', errorText(e))
    }
  }

  return (
    <Section title="Waktu caption">
      {!voiced.length ? (
        <p className="text-[13px] text-ink-2">Caption muncul setelah suara narasi dibuat.</p>
      ) : running ? (
        <span className="text-[13px] text-sun-ink">
          {job!.message ?? 'Menyinkronkan'} {Math.round(job!.progress * 100)}%
        </span>
      ) : estimated === 0 ? (
        <p className="text-[13px] text-ok-ink">Semua caption mengikuti waktu suara aslinya.</p>
      ) : (
        <>
          <p className="text-[13px] leading-relaxed text-ink-2">
            {estimated} klip masih memakai waktu perkiraan, jadi caption bisa sedikit mendahului atau tertinggal dari suaranya.
          </p>
          {whisper ? (
            <Button size="sm" className="self-start" icon={<AudioLines className="size-4" />} onClick={() => void sync()}>
              Sinkronkan dengan Whisper
            </Button>
          ) : (
            <p className="text-[13px] text-ink-2">
              Unduh model Whisper Small di{' '}
              <button
                type="button"
                onClick={async () => {
                  try {
                    await flush()
                    go({ name: 'settings', back: { name: 'project', id: project.id } })
                  } catch (e) {
                    toast('error', errorText(e))
                  }
                }}
                className="font-semibold text-accent-dark hover:underline"
              >
                Pengaturan
              </button>{' '}
              untuk menyamakannya dengan suara.
            </p>
          )}
        </>
      )}
    </Section>
  )
}

const TEXT_SWATCHES = ['#FFFFFF', '#1F1D1A', '#FFC93C', '#E4572E', '#4DABF7', '#51CF66', '#F783AC']
const BOX_SWATCHES = ['#1F1D1A', '#FFFFFF', '#C93A1B', '#FFC93C', '#1E4A7A', '#25572B']

/** A row of colour dots plus a free colour picker. */
function Swatches({ value, colors, label, onChange }: { value: string | null; colors: string[]; label: string; onChange: (c: string) => void }) {
  const custom = !!value && !colors.some((c) => c.toLowerCase() === value.toLowerCase())
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {colors.map((c) => {
        const on = value?.toLowerCase() === c.toLowerCase()
        return (
          <button
            key={c}
            type="button"
            aria-label={`${label} ${c}`}
            aria-pressed={on}
            onClick={() => onChange(c)}
            className={cx('size-7 rounded-full border', on ? 'border-ink ring-2 ring-accent ring-offset-2 ring-offset-surface' : 'border-line-3')}
            style={{ background: c }}
          />
        )
      })}
      <label
        title="Warna lain"
        className={cx(
          'relative size-7 cursor-pointer overflow-hidden rounded-full border',
          custom ? 'border-ink ring-2 ring-accent ring-offset-2 ring-offset-surface' : 'border-line-3'
        )}
        style={{ background: custom ? value! : 'conic-gradient(#e03131, #ffd43b, #51cf66, #22b8cf, #4c6ef5, #cc5de8, #e03131)' }}
      >
        <input
          type="color"
          value={value ?? '#ffffff'}
          onChange={(ev) => onChange(ev.target.value.toUpperCase())}
          aria-label={`${label} lainnya`}
          className="absolute inset-0 size-full cursor-pointer opacity-0"
        />
      </label>
    </div>
  )
}

/** Font, colours and box on top of the chosen caption style. */
function CaptionCustom() {
  const { project, updateEditor } = useProject()
  if (!project) return null
  const e = project.editor
  const r = resolveCaption(e)
  if (!r) return null
  return (
    <>
      <Section title="Font">
        <Select
          label="Font caption"
          size="sm"
          value={getCaptionFont(e.captionFont).id}
          onChange={(v) => updateEditor({ captionFont: v })}
          options={CAPTION_FONTS.map((f) => ({
            value: f.id,
            label: f.name,
            group: f.group,
            // Each option is written in its own font.
            labelStyle: { fontFamily: `'${f.family}', sans-serif`, fontSize: 17, fontWeight: f.id === 'poppins' ? 700 : 400 }
          }))}
        />
      </Section>
      <Section title="Warna teks">
        <Swatches value={r.color} colors={TEXT_SWATCHES} label="Warna teks" onChange={(c) => updateEditor({ captionColor: c })} />
        <span className="text-xs text-muted">Kata yang sedang diucapkan</span>
        <Swatches value={r.highlight} colors={TEXT_SWATCHES} label="Warna sorotan" onChange={(c) => updateEditor({ captionHighlight: c })} />
      </Section>
      <Section title="Latar caption">
        <Segmented
          size="sm"
          value={r.box ? 'on' : 'off'}
          onChange={(v) => updateEditor({ captionBg: v === 'off' ? null : { color: r.box ?? '#1F1D1A', opacity: r.box ? r.boxAlpha : 0.85 } })}
          options={[
            { id: 'on', label: 'Kotak' },
            { id: 'off', label: 'Tanpa latar' }
          ]}
          className="self-start"
        />
        {r.box ? (
          <>
            <Swatches value={r.box} colors={BOX_SWATCHES} label="Warna latar" onChange={(c) => updateEditor({ captionBg: { color: c, opacity: r.boxAlpha } })} />
            <label className="flex items-center gap-3 text-[13px]">
              <span className="w-16 text-ink-2">Opasitas</span>
              <input
                type="range"
                min={15}
                max={100}
                step={5}
                value={Math.round(r.boxAlpha * 100)}
                onChange={(ev) => updateEditor({ captionBg: { color: r.box!, opacity: Number(ev.target.value) / 100 } })}
                aria-label="Opasitas latar caption"
                className="flex-1"
              />
              <span className="w-10 text-right font-mono">{Math.round(r.boxAlpha * 100)}%</span>
            </label>
          </>
        ) : (
          <span className="text-xs text-muted">Tanpa latar, teks diberi garis tepi supaya tetap terbaca.</span>
        )}
      </Section>
    </>
  )
}

function CaptionTab() {
  const { project, updateEditor } = useProject()
  if (!project) return null
  const e = project.editor
  return (
    <>
      <CaptionTiming />
      <Section title="Gaya caption">
        <div className="grid grid-cols-2 gap-2">
          {CAPTION_STYLES.map((s) => {
            const on = e.captionStyle === s.id
            return (
              <button key={s.id} type="button" aria-pressed={on} onClick={() => updateEditor({ captionStyle: s.id, captionColor: undefined, captionHighlight: undefined, captionBg: undefined })} className="flex flex-col gap-1 text-left">
                <span
                  className={cx('flex h-[52px] w-full items-center justify-center rounded-[10px] bg-[#E99A5B]', on ? 'border-2 border-accent' : 'border border-ink')}
                >
                  <span
                    className="whitespace-nowrap rounded-md px-1.5 py-0.5 text-[12px]"
                    style={{
                      fontFamily: 'Poppins, sans-serif',
                      fontWeight: s.weight,
                      color: s.color,
                      background: s.box ?? 'transparent',
                      WebkitTextStroke: s.box ? undefined : `0.8px ${s.outlineColor}`,
                      paintOrder: 'stroke fill'
                    }}
                  >
                    {s.uppercase ? 'NUSANTARA' : 'Nusantara'} <span style={{ color: s.highlight }}>{s.uppercase ? 'BERSATU' : 'bersatu'}</span>
                  </span>
                </span>
                <span className="text-xs font-medium text-ink-2">{s.name}</span>
              </button>
            )
          })}
        </div>
        <button
          type="button"
          aria-pressed={e.captionStyle === 'none'}
          onClick={() => updateEditor({ captionStyle: 'none' })}
          className={cx(
            'flex h-9 items-center justify-center gap-1.5 rounded-[10px] text-[13px] font-medium',
            e.captionStyle === 'none' ? 'border-2 border-accent bg-accent-soft' : 'border border-line-2 hover:border-ink-2'
          )}
        >
          <X className="size-3.5" />
          Tanpa caption
        </button>
      </Section>
      {e.captionStyle !== 'none' && <CaptionCustom />}
      <Section title="Posisi">
        <Segmented value={e.captionPosition} onChange={(v) => updateEditor({ captionPosition: v })} options={CAPTION_POSITIONS} />
      </Section>
      <Section title="Ukuran caption" className="border-b-0">
        <label className="flex items-center gap-3 text-[13px]">
          <span className="text-xs text-muted">A</span>
          <input
            type="range"
            min={50}
            max={180}
            step={5}
            value={Math.round(captionScaleOf(e) * 100)}
            onChange={(ev) => updateEditor({ captionScale: Number(ev.target.value) / 100 })}
            aria-label="Ukuran caption"
            className="flex-1"
          />
          <span className="text-base font-semibold text-muted">A</span>
          <span className="w-12 text-right font-mono">{Math.round(captionScaleOf(e) * 100)}%</span>
        </label>
        <button type="button" onClick={() => updateEditor({ captionScale: 1, captionSize: 'sedang' })} className="self-start text-xs font-semibold text-accent-dark hover:underline">
          Kembalikan ke 100%
        </button>
      </Section>
    </>
  )
}

/** One field of the music prompt with a copy button. */
function PromptField({ label, hint, value, rows, onChange }: { label: string; hint?: string; value: string; rows: number; onChange: (v: string) => void }) {
  const [copied, setCopied] = useState(false)
  useEffect(() => {
    if (!copied) return
    const t = setTimeout(() => setCopied(false), 1500)
    return () => clearTimeout(t)
  }, [copied])
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2">
        <span className="text-[13px] font-semibold">{label}</span>
        {hint && <span className="text-xs text-muted">{hint}</span>}
        <span className="flex-1" />
        <button
          type="button"
          onClick={() => void window.api.app.copyText(value).then(() => setCopied(true))}
          className={cx('inline-flex h-7 items-center gap-1 rounded-lg px-2 text-xs font-semibold', copied ? 'text-ok-ink' : 'text-accent-dark hover:bg-accent-soft')}
        >
          {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
          {copied ? 'Tersalin' : 'Salin'}
        </button>
      </div>
      <AutoTextarea
        minRows={rows}
        maxHeight={220}
        value={value}
        onChange={(ev) => onChange(ev.target.value)}
        aria-label={label}
        className={cx(inputCls, 'py-2 text-[13px] leading-relaxed')}
      />
    </div>
  )
}

const MUSIC_SITES = [
  { name: 'Suno', url: 'https://suno.com/create' },
  { name: 'Udio', url: 'https://www.udio.com/create' },
  { name: 'ElevenLabs Music', url: 'https://elevenlabs.io/app/music' }
]

/** Writes a prompt for background music that suits this story, to use in Suno or another music AI. */
function MusicPromptSection() {
  const { project, updateEditor, flush } = useProject()
  const { toast } = useApp()
  const [busy, setBusy] = useState(false)
  if (!project) return null
  const mp = project.editor.musicPrompt ?? null
  const set = (patch: Partial<NonNullable<typeof mp>>): void => {
    if (mp) updateEditor({ musicPrompt: { ...mp, ...patch } })
  }
  const make = async (): Promise<void> => {
    setBusy(true)
    try {
      await flush()
      const r = await window.api.story.musicPrompt(project.id)
      updateEditor({ musicPrompt: r })
    } catch (e) {
      toast('error', errorText(e))
    } finally {
      setBusy(false)
    }
  }
  return (
    <Section title="Prompt musik AI">
      <p className="text-[13px] leading-relaxed text-ink-2">
        {mp?.reason || 'Buat prompt musik latar yang cocok dengan cerita ini, lalu tempel di Suno atau AI musik lain. Hasilnya bisa dipilih lewat tombol Pilih musik di bawah.'}
      </p>
      {mp && (
        <>
          <PromptField label="Style of Music" hint="kolom gaya di Suno" value={mp.style} rows={3} onChange={(v) => set({ style: v })} />
          <PromptField label="Exclude styles" hint="yang dihindari" value={mp.exclude} rows={1} onChange={(v) => set({ exclude: v })} />
          <PromptField label="Judul lagu" value={mp.title} rows={1} onChange={(v) => set({ title: v })} />
          <PromptField label="Deskripsi" hint="Udio, ElevenLabs, dll." value={mp.description} rows={5} onChange={(v) => set({ description: v })} />
          <p className="text-xs text-muted">
            Pilih mode Instrumental. Video sekitar {secLabel(mp.durationSec * 1000)}; saat ekspor musik otomatis diulang atau dipotong sesuai durasi, lalu memudar di akhir.
          </p>
          <div className="flex flex-wrap gap-1.5">
            {MUSIC_SITES.map((s) => (
              <button
                key={s.name}
                type="button"
                onClick={() => void window.api.app.openExternal(s.url)}
                className="inline-flex h-8 items-center gap-1.5 rounded-full border border-line-2 px-3 text-xs font-semibold text-ink-2 hover:border-ink-2 hover:text-ink"
              >
                Buka {s.name}
                <ExternalLink className="size-3.5" />
              </button>
            ))}
          </div>
        </>
      )}
      <Button size="sm" variant={mp ? 'secondary' : 'accent'} className="self-start" icon={<Sparkles className="size-4" />} loading={busy} onClick={() => void make()}>
        {mp ? 'Buat ulang prompt' : 'Buat prompt musik'}
      </Button>
    </Section>
  )
}

function MusicTab() {
  const { project, assets, updateEditor } = useProject()
  const { toast } = useApp()
  if (!project) return null
  const e = project.editor
  const music = e.musicAssetId ? assets[e.musicAssetId] : null
  const pick = async (): Promise<void> => {
    try {
      const a = await window.api.assets.pickMusic(project.id)
      if (a) {
        useProject.getState().addAsset(a)
        updateEditor({ musicAssetId: a.id })
      }
    } catch (err) {
      toast('error', errorText(err))
    }
  }
  return (
    <>
      <MusicPromptSection />
      <Section title="Musik latar">
        {music ? (
          <div className="flex items-center gap-2.5 rounded-xl border border-[#9CC596] bg-ok-soft px-3 py-2.5 text-[13px] text-ok-ink">
            <Music className="size-4 shrink-0" />
            <span className="flex-1 truncate">{music.prompt}</span>
            <button type="button" aria-label="Lepas musik" onClick={() => updateEditor({ musicAssetId: null })} className="hover:text-ink">
              <X className="size-4" />
            </button>
          </div>
        ) : (
          <p className="text-[13px] text-ink-2">Belum ada musik. Pilih file MP3 atau WAV dari komputer.</p>
        )}
        <Button size="sm" className="self-start" icon={<Music className="size-4" />} onClick={() => void pick()}>
          {music ? 'Ganti musik' : 'Pilih musik'}
        </Button>
      </Section>
      {music && (
        <Section title="Volume" className="border-b-0">
          <label className="flex items-center gap-3 text-[13px]">
            <input
              type="range"
              min={-36}
              max={-3}
              step={1}
              value={e.musicVolumeDb}
              onChange={(ev) => updateEditor({ musicVolumeDb: Number(ev.target.value) })}
              aria-label="Volume musik"
              className="flex-1"
            />
            <span className="w-14 text-right font-mono">{e.musicVolumeDb} dB</span>
          </label>
          <div className="flex items-center gap-3">
            <div className="flex-1">
              <p className="text-sm font-semibold">Kecilkan saat narasi</p>
              <p className="text-[13px] text-muted">Musik otomatis turun ketika ada suara</p>
            </div>
            <Switch on={e.duckMusic} onChange={(v) => updateEditor({ duckMusic: v })} label="Kecilkan musik saat narasi" />
          </div>
        </Section>
      )}
    </>
  )
}

export function Inspector({ tab, clip, index, total }: { tab: InspectorTab; clip: Clip | null; index: number; total: number }) {
  return (
    <aside className="scroll-thin flex min-h-0 flex-col overflow-y-auto border-l border-line bg-surface">
      {tab === 'klip' && (clip ? <ClipTab clip={clip} index={index} /> : <p className="p-5 text-sm text-muted">Pilih klip di timeline.</p>)}
      {tab === 'caption' && <CaptionTab />}
      {tab === 'overlay' && <OverlayPanel total={total} />}
      {tab === 'musik' && <MusicTab />}
    </aside>
  )
}
