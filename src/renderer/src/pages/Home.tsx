import { useEffect, useMemo, useState } from 'react'
import { AlertTriangle, ArrowRight, Copy, FolderOpen, MoreHorizontal, Plus, Search, SlidersHorizontal, Trash2 } from 'lucide-react'
import { DURATIONS, LANGUAGES, llmName } from '@shared/models'
import { getStyle } from '@shared/styles'
import type { ApiProvider, AppSettings, AspectRatio, KeyStatus, ProjectStatus, ProjectSummary, TtsProvider } from '@shared/types'
import { Logo } from '../components/Logo'
import { Select, type SelectOption } from '../components/Select'
import { StyleSwatch } from '../components/StyleArt'
import { StyleMenu } from '../components/StyleMenu'
import { Badge, Button, Chip, IconButton, Menu, confirmDialog, cx } from '../components/ui'
import { errorText, mmss, relativeTime } from '../lib/format'
import { TTS_NAMES, ttsReadiness, usableTts } from '../lib/keys'
import { useApp } from '../store/app'

const STATUS: Record<ProjectStatus, { label: string; tone: 'draft' | 'info' | 'warn' | 'ok' }> = {
  draft: { label: 'Draf', tone: 'draft' },
  script: { label: 'Naskah', tone: 'info' },
  storyboard: { label: 'Storyboard', tone: 'info' },
  editing: { label: 'Editing', tone: 'warn' },
  exported: { label: 'Diekspor', tone: 'ok' }
}

const FILTERS: { id: 'all' | 'draft' | 'progress' | 'exported'; label: string; match: (s: ProjectStatus) => boolean }[] = [
  { id: 'all', label: 'Semua', match: () => true },
  { id: 'draft', label: 'Draf', match: (s) => s === 'draft' },
  { id: 'progress', label: 'Dalam proses', match: (s) => s === 'script' || s === 'storyboard' || s === 'editing' },
  { id: 'exported', label: 'Diekspor', match: (s) => s === 'exported' }
]

const SUGGESTIONS = [
  'Asal-usul Sumpah Palapa dan bagaimana Gajah Mada menepatinya',
  'Misteri Segitiga Bermuda: fakta dan mitosnya',
  'Kenapa kucing suka masuk ke dalam kotak?'
]

/** "a, b dan c" */
function joinList(items: string[]): string {
  return items.length < 2 ? items.join('') : `${items.slice(0, -1).join(', ')} dan ${items[items.length - 1]}`
}

const ASPECTS: SelectOption<AspectRatio>[] = [
  { value: '16:9', label: '16:9 · YouTube', icon: <span className="block h-[10px] w-[17px] rounded-[2px] border-[1.5px] border-ink" /> },
  { value: '9:16', label: '9:16 · Shorts', icon: <span className="mx-[3px] block h-[16px] w-[10px] rounded-[2px] border-[1.5px] border-ink" /> }
]

export function Home() {
  const { go, toast } = useApp()
  const [projects, setProjects] = useState<ProjectSummary[] | null>(null)
  const [keys, setKeys] = useState<KeyStatus[]>([])
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null)
  const [filter, setFilter] = useState<(typeof FILTERS)[number]['id']>('all')
  const [query, setQuery] = useState('')
  const [synopsis, setSynopsis] = useState('')
  const [durationSec, setDurationSec] = useState(60)
  const [language, setLanguage] = useState('id')
  const [aspectRatio, setAspectRatio] = useState<AspectRatio>('16:9')
  const [styleId, setStyleId] = useState('stickman')
  const [tts, setTts] = useState<TtsProvider>('gemini')
  const [creating, setCreating] = useState(false)

  const refresh = (): void => {
    window.api.projects
      .list()
      .then(setProjects)
      .catch((e) => toast('error', errorText(e)))
  }

  useEffect(() => {
    refresh()
    void window.api.settings.keys().then(setKeys)
    void window.api.settings.get().then((s) => {
      setAppSettings(s)
      setTts(s.defaultTtsProvider)
      setLanguage(s.defaultLanguage)
    })
  }, [])

  // Start on a narrator service that has a working key.
  useEffect(() => {
    if (appSettings && keys.length) setTts(usableTts(appSettings.defaultTtsProvider, keys))
  }, [appSettings, keys])

  const ttsOptions = useMemo((): SelectOption<TtsProvider>[] => {
    const ready = ttsReadiness(keys)
    return (['gemini', 'elevenlabs'] as const).map((p) => {
      const blocked = keys.length > 0 && !ready[p].ok
      return { value: p, label: TTS_NAMES[p], disabled: blocked, note: blocked ? `${ready[p].reason}. Atur di Pengaturan.` : undefined }
    })
  }, [keys])

  const setupIssues = useMemo(() => {
    if (!appSettings || !keys.length) return []
    // A key counts once it is saved and did not fail its last check.
    const ok = (p: ApiProvider): boolean => {
      const k = keys.find((x) => x.provider === p)
      return !!k?.configured && k.lastOk !== false
    }
    const issues: { what: string; blocks: string }[] = []
    if (!ok('higgsfield') && !ok('flow2api')) {
      issues.push({ what: 'kunci Higgsfield atau Google Flow (Flow2API)', blocks: 'gambar & video' })
    }
    const llm = appSettings.llmProvider
    if (!ok(llm)) issues.push({ what: `kunci ${llmName(llm)}`, blocks: 'cerita' })
    else if (!appSettings.llmModels[llm]) issues.push({ what: `model ${llmName(llm)}`, blocks: 'cerita' })
    // Narration needs just one working voice service; every project can pick either one.
    const tts = ttsReadiness(keys)
    if (!tts.gemini.ok && !tts.elevenlabs.ok) issues.push({ what: 'kunci Gemini atau ElevenLabs', blocks: 'suara narator' })
    return issues
  }, [keys, appSettings])

  const visible = useMemo(() => {
    const f = FILTERS.find((x) => x.id === filter)!
    const q = query.trim().toLowerCase()
    return (projects ?? []).filter((p) => f.match(p.status) && (!q || p.title.toLowerCase().includes(q)))
  }, [projects, filter, query])

  const start = async (): Promise<void> => {
    setCreating(true)
    try {
      const b = await window.api.projects.create({ synopsis, durationSec, language, aspectRatio, styleId, ttsProvider: tts })
      go({ name: 'project', id: b.project.id })
    } catch (e) {
      toast('error', errorText(e))
      setCreating(false)
    }
  }

  const remove = async (p: ProjectSummary): Promise<void> => {
    const ok = await confirmDialog({
      title: 'Hapus proyek?',
      body: `“${p.title}” beserta semua gambar, video, dan suaranya akan dihapus dari komputer ini. Tindakan ini tidak bisa dibatalkan.`,
      confirm: 'Hapus proyek',
      danger: true
    })
    if (!ok) return
    await window.api.projects.remove(p.id)
    refresh()
  }

  const duplicate = async (p: ProjectSummary): Promise<void> => {
    try {
      await window.api.projects.duplicate(p.id)
      refresh()
    } catch (e) {
      toast('error', errorText(e))
    }
  }

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-[68px] shrink-0 items-center gap-6 border-b border-line bg-surface px-8">
        <Logo />
        <label className="flex h-10 w-[360px] items-center gap-2 rounded-full border border-line bg-sand px-3.5 text-muted">
          <Search className="size-[18px]" />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Cari proyek"
            aria-label="Cari proyek"
            className="flex-1 bg-transparent text-sm text-ink outline-none"
          />
        </label>
        <div className="flex-1" />
        <Button icon={<SlidersHorizontal className="size-[18px]" />} onClick={() => go({ name: 'settings', back: { name: 'home' } })}>
          Pengaturan
          {setupIssues.length > 0 && <span className="size-2 rounded-full bg-accent" />}
        </Button>
      </header>

      <main className="scroll-thin flex-1 overflow-y-auto">
        <div className="mx-auto flex max-w-[1280px] flex-col gap-5 px-12 pt-11">
          {setupIssues.length > 0 && (
            <div className="flex items-center gap-3 rounded-2xl border border-sun bg-sun-soft px-4 py-3 text-sm text-sun-ink">
              <AlertTriangle className="size-[18px] shrink-0" />
              <span className="flex-1">
                Sebelum mulai, atur dulu {joinList(setupIssues.map((i) => i.what))} di Pengaturan. Tanpa itu,{' '}
                {joinList([...new Set(setupIssues.map((i) => i.blocks))])} belum bisa dibuat.
              </span>
              <Button size="sm" variant="secondary" onClick={() => go({ name: 'settings', back: { name: 'home' } })}>
                Atur sekarang
              </Button>
            </div>
          )}
          <div>
            <h1 className="font-display text-[46px] font-bold leading-[1.08] tracking-[-0.025em]">Mau bikin video apa hari ini?</h1>
            <p className="mt-3 text-[17px] text-ink-2">
              Tulis idenya. AI menyusun klip, naskah, dan suaranya, lalu kamu tinggal merapikan di editor.
            </p>
          </div>
          <div className="flex flex-col gap-3 rounded-[18px] border-[1.5px] border-ink bg-surface py-5 pl-6 pr-5 shadow-ink-lg">
            <label htmlFor="ide" className="text-[13px] font-semibold text-ink-2">
              Ide cerita atau sinopsis
            </label>
            <textarea
              id="ide"
              rows={3}
              value={synopsis}
              onChange={(e) => setSynopsis(e.target.value)}
              placeholder="Contoh: Gajah Mada bersumpah tidak akan menikmati palapa sebelum Nusantara bersatu. Para menteri menertawakannya, tapi dia membuktikannya…"
              className="resize-none bg-transparent text-lg leading-relaxed outline-none placeholder:text-muted/70"
            />
            <div className="flex flex-wrap items-center gap-2 border-t border-dashed border-line pt-3.5">
              <Select
                variant="pill"
                label="Durasi"
                value={durationSec}
                onChange={setDurationSec}
                options={DURATIONS.map((d) => ({ value: d.sec, label: d.label }))}
              />
              <Select
                variant="pill"
                label="Bahasa"
                value={language}
                onChange={setLanguage}
                options={LANGUAGES.map((l) => ({ value: l.code, label: l.label }))}
                menuWidth={200}
              />
              <Select variant="pill" label="Format" value={aspectRatio} onChange={setAspectRatio} options={ASPECTS} menuWidth={200} />
              <StyleMenu value={styleId} onChange={setStyleId} />
              <Select variant="pill" label="Suara narator" value={tts} onChange={setTts} options={ttsOptions} menuWidth={280} />
              <div className="flex-1" />
              <Button variant="primary" size="lg" loading={creating} onClick={() => void start()}>
                Mulai
                <ArrowRight className="size-[18px]" />
              </Button>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2 text-[13px] text-ink-2">
            <span>Coba ide ini:</span>
            {SUGGESTIONS.map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => setSynopsis(s)}
                className="h-9 rounded-full border border-dashed border-line-3 px-3.5 text-[13px] text-ink-2 hover:border-ink-2 hover:text-ink"
              >
                {s}
              </button>
            ))}
          </div>
        </div>

        <section className="mx-auto flex max-w-[1280px] flex-col gap-[18px] px-12 pb-12 pt-11">
          <div className="flex items-center gap-4">
            <h2 className="font-display text-[27px] font-bold tracking-[-0.015em]">Proyek kamu</h2>
            <div className="ml-2 flex gap-1.5">
              {FILTERS.map((f) => {
                const count = (projects ?? []).filter((p) => f.match(p.status)).length
                return (
                  <Chip key={f.id} on={filter === f.id} onClick={() => setFilter(f.id)}>
                    {f.label}
                    <span className={cx('text-xs', filter === f.id ? 'text-line-2' : 'text-muted')}>{count}</span>
                  </Chip>
                )
              })}
            </div>
          </div>

          <div className="grid grid-cols-4 gap-5">
            <button
              type="button"
              onClick={() => void start()}
              className="flex min-h-[262px] flex-col items-center justify-center gap-2.5 rounded-2xl border-[1.5px] border-dashed border-line-3 text-ink-2 hover:border-ink-2 hover:bg-surface"
            >
              <span className="flex size-[54px] items-center justify-center rounded-full border-[1.5px] border-ink bg-accent text-white shadow-ink">
                <Plus className="size-6" strokeWidth={2.2} />
              </span>
              <span className="text-base font-semibold text-ink">Proyek baru</span>
              <span className="text-[13px]">Mulai dari ide atau sinopsis</span>
            </button>

            {visible.map((p) => {
              const st = STATUS[p.status]
              return (
                <article key={p.id} className="flex flex-col overflow-hidden rounded-2xl border border-line bg-surface">
                  <button type="button" onClick={() => go({ name: 'project', id: p.id })} className="text-left">
                    <div className="relative aspect-video w-full overflow-hidden border-b border-line">
                      {p.coverUrl ? (
                        <img src={p.coverUrl} alt="" className="size-full object-cover" />
                      ) : (
                        <StyleSwatch styleId={p.styleId} className="size-full" iconSize={40} />
                      )}
                      <span className="absolute left-2.5 top-2.5 rounded-full border border-ink bg-surface px-2.5 py-[3px] text-xs font-semibold">
                        {getStyle(p.styleId).name}
                      </span>
                      {p.totalMs > 0 && (
                        <span className="absolute bottom-2.5 right-2.5 rounded-md bg-ink px-2 py-[3px] font-mono text-xs text-paper">
                          {mmss(p.totalMs)}
                        </span>
                      )}
                    </div>
                    <div className="line-clamp-2 min-h-[46px] px-4 pb-0.5 pt-3.5 text-base font-semibold leading-snug">{p.title}</div>
                  </button>
                  <div className="flex items-center gap-2 py-2 pl-4 pr-2">
                    <Badge tone={st.tone}>{st.label}</Badge>
                    <span className="truncate text-[13px] text-muted">{relativeTime(p.updatedAt)}</span>
                    <div className="flex-1" />
                    <Menu
                      trigger={(open) => (
                        <IconButton label="Opsi proyek" onClick={open} className="border-transparent" size={36}>
                          <MoreHorizontal className="size-[18px]" />
                        </IconButton>
                      )}
                      items={[
                        { label: 'Buka', icon: <FolderOpen className="size-4" />, run: () => go({ name: 'project', id: p.id }) },
                        { label: 'Duplikat', icon: <Copy className="size-4" />, run: () => void duplicate(p) },
                        { label: 'Hapus', icon: <Trash2 className="size-4" />, danger: true, run: () => void remove(p) }
                      ]}
                    />
                  </div>
                </article>
              )
            })}
          </div>
          {projects && projects.length === 0 && (
            <p className="text-sm text-muted">Belum ada proyek. Tulis ide di atas lalu klik Mulai.</p>
          )}

          <footer className="mt-14 flex flex-col items-center justify-center gap-1 border-t border-line/60 pt-6 pb-2 text-xs text-muted">
            <div>
              <span className="font-semibold text-ink-2">De Story v1.1.0</span> · Modified &amp; Developed by{' '}
              <span className="font-semibold text-ink-2">ikram.mf7</span>
            </div>
            <div>
              Based on original project:{' '}
              <span className="font-medium text-ink-2">Bang Story</span> by{' '}
              <span className="font-medium text-ink-2">Bang Tutorial</span>
            </div>
          </footer>
        </section>
      </main>
    </div>
  )
}
