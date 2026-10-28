import { useEffect, useRef, useState, type ReactNode } from 'react'
import {
  ArrowLeft,
  Check,
  ChevronDown,
  Copy,
  Cpu,
  ExternalLink,
  Eye,
  EyeOff,
  Film,
  FolderOpen,
  Image as ImageIcon,
  Info,
  KeyRound,
  Mic,
  Server,
  SlidersHorizontal,
  Sparkles,
  Trash2
} from 'lucide-react'
import { FLOW_IMAGE_MODELS, FLOW_VIDEO_MODELS } from '@shared/flow'
import { HF_IMAGE_MODELS, HF_VIDEO_MODELS } from '@shared/higgsfield'
import { LANGUAGES, LLM_PROVIDERS } from '@shared/models'
import type {
  ApiProvider,
  AppSettings,
  Flow2ApiExtensionInfo,
  Flow2ApiPanelInfo,
  Flow2ApiStatus,
  GeminiKeyTestResult,
  KeyStatus,
  KeyTestResult,
  LlmProvider,
  TtsProvider
} from '@shared/types'
import { Logo } from '../components/Logo'
import { ModelPicker } from '../components/ModelPicker'
import { ProviderLogo, type ProviderLogoId } from '../components/ProviderLogo'
import { Select } from '../components/Select'
import { WhisperSection } from './WhisperSection'
import { Badge, Button, Field, IconButton, confirmDialog, cx, inputCls } from '../components/ui'
import { errorText, relativeTime } from '../lib/format'
import { useModels } from '../lib/useModels'
import { useApp } from '../store/app'

type Tab = 'services' | 'general' | 'about'

interface ProviderOption<T extends string> {
  id: T
  name: string
  note: string
  logo: ProviderLogoId
  status?: KeyStatus
}

function StatusBadge({ status }: { status?: KeyStatus }) {
  if (!status?.configured) return <Badge tone="draft">Belum diatur</Badge>
  if (status.lastOk === false) return <Badge tone="bad">Gagal terhubung</Badge>
  if (status.lastOk)
    return (
      <Badge tone="ok">
        <Check className="size-3" strokeWidth={3} />
        Terhubung
      </Badge>
    )
  return <Badge tone="info">Tersimpan</Badge>
}

/** One dropdown to choose which provider a section uses; only that provider's settings are shown below it. */
function ProviderSelect<T extends string>({ value, options, onChange }: { value: T; options: ProviderOption<T>[]; onChange: (id: T) => void }) {
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    const onDown = (e: MouseEvent): void => {
      if (!root.current?.contains(e.target as Node)) setOpen(false)
    }
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') setOpen(false)
    }
    window.addEventListener('mousedown', onDown)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('mousedown', onDown)
      window.removeEventListener('keydown', onKey)
    }
  }, [open])
  const cur = options.find((o) => o.id === value) ?? options[0]
  return (
    <div ref={root} className="relative">
      <button
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className={cx(
          'flex w-full items-center gap-3 rounded-2xl border-[1.5px] border-ink bg-surface px-4 py-3 text-left transition-shadow',
          open ? 'shadow-none' : 'shadow-ink hover:shadow-ink-md'
        )}
      >
        <ProviderLogo id={cur.logo} />
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="text-base font-bold">{cur.name}</span>
          <span className="truncate text-[13px] text-ink-2">{cur.note}</span>
        </span>
        <StatusBadge status={cur.status} />
        <ChevronDown className={cx('size-5 shrink-0 transition-transform', open && 'rotate-180')} />
      </button>
      {open && (
        <div role="listbox" className="absolute inset-x-0 top-full z-40 mt-1.5 flex flex-col gap-0.5 rounded-2xl border-[1.5px] border-ink bg-surface p-1.5 shadow-ink-lg">
          {options.map((o) => {
            const on = o.id === value
            return (
              <button
                key={o.id}
                type="button"
                role="option"
                aria-selected={on}
                onClick={() => {
                  setOpen(false)
                  if (!on) onChange(o.id)
                }}
                className={cx('flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left', on ? 'bg-accent-soft' : 'hover:bg-sand')}
              >
                <ProviderLogo id={o.logo} size={36} />
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="text-[15px] font-semibold">{o.name}</span>
                  <span className="truncate text-[13px] text-ink-2">{o.note}</span>
                </span>
                <StatusBadge status={o.status} />
                <span className="flex size-5 shrink-0 items-center justify-center">{on && <Check className="size-4 text-accent" strokeWidth={2.6} />}</span>
              </button>
            )
          })}
        </div>
      )}
    </div>
  )
}

/** API key field(s) with save, test and delete. The custom endpoint also takes a base URL. */
function KeyForm({
  provider,
  name,
  status,
  label = 'API key',
  placeholder = 'Tempel API key',
  hint,
  help,
  link,
  linkLabel,
  customUrl,
  flow2apiMode,
  onChanged,
  onSaved
}: {
  provider: ApiProvider
  name: string
  status?: KeyStatus
  label?: string
  placeholder?: string
  hint?: ReactNode
  help: string
  link: string | null
  linkLabel: string
  customUrl?: string
  /** Flow2API only: which backend this app talks to. */
  flow2apiMode?: 'bundled' | 'external'
  onChanged: () => void | Promise<void>
  onSaved?: (r: KeyTestResult) => void
}) {
  const { toast } = useApp()
  const configured = !!status?.configured
  const isEndpoint = provider === 'custom' || provider === 'flow2api'
  const preview = status?.preview ?? null
  const [editing, setEditing] = useState(!configured)
  const [key, setKey] = useState('')
  const [url, setUrl] = useState(customUrl ?? (provider === 'flow2api' ? 'http://localhost:8000' : ''))
  const [revealed, setRevealed] = useState<string | null>(null)
  const [showTyped, setShowTyped] = useState(false)
  const [busy, setBusy] = useState<'save' | 'test' | null>(null)
  const [mode, setMode] = useState<'bundled' | 'external'>(flow2apiMode ?? 'bundled')

  useEffect(() => {
    if (flow2apiMode !== undefined) {
      setMode(flow2apiMode)
    }
  }, [flow2apiMode])

  // A newly saved or removed key puts the field back in its resting state.
  useEffect(() => {
    setEditing(!configured)
    setKey('')
    setRevealed(null)
  }, [configured, preview])

  useEffect(() => {
    if (customUrl !== undefined) {
      setUrl(customUrl)
    }
  }, [customUrl])

  const showStoredKey = !editing && !!preview
  const showStoredNoKey = isEndpoint && !editing && configured && !preview
  const urlChanged = isEndpoint && url.trim() !== (customUrl ?? '').trim()
  const modeChanged = provider === 'flow2api' && mode !== flow2apiMode
  const canSave = isEndpoint
    ? (!!url.trim() && (urlChanged || modeChanged || !!key.trim() || editing || !configured)) ||
      (provider === 'flow2api' && modeChanged && !url.trim())
    : editing && !!key.trim()
  // A pasted key only counts once it is saved; Enter saves it too.
  const unsaved = editing && !!key.trim() && busy !== 'save'
  const onEnter = (e: React.KeyboardEvent<HTMLInputElement>): void => {
    if (e.key !== 'Enter' || !canSave || busy) return
    e.preventDefault()
    void save()
  }

  const save = async (): Promise<void> => {
    setBusy('save')
    try {
      const r =
        provider === 'custom'
          ? await window.api.settings.setCustom(url, key)
          : provider === 'flow2api'
            ? await window.api.settings.setFlow2api(url, key, mode)
            : await window.api.settings.setKey(provider, key)
      toast(r.ok ? 'success' : 'error', `${name}: ${r.message}`)
      setKey('')
      setEditing(false)
      await onChanged()
      onSaved?.(r)
    } catch (e) {
      toast('error', errorText(e))
    } finally {
      setBusy(null)
    }
  }

  const toggleEye = async (): Promise<void> => {
    if (!showStoredKey) {
      setShowTyped(!showTyped)
      return
    }
    setRevealed(revealed ? null : await window.api.settings.revealKey(provider))
  }
  const visible = showStoredKey ? !!revealed : showTyped

  const test = async (): Promise<void> => {
    setBusy('test')
    try {
      const r = await window.api.settings.testKey(provider)
      toast(r.ok ? 'success' : 'error', `${name}: ${r.message}`)
      await onChanged()
    } finally {
      setBusy(null)
    }
  }

  const clear = async (): Promise<void> => {
    const ok = await confirmDialog({
      title: `Hapus ${isEndpoint ? 'endpoint' : 'kunci'} ${name}?`,
      body: 'Data ini akan dihapus dari komputer ini. Kamu bisa menambahkannya lagi kapan saja.',
      confirm: 'Hapus',
      danger: true
    })
    if (!ok) return
    await window.api.settings.clearKey(provider)
    setUrl(provider === 'flow2api' ? 'http://localhost:8000' : '')
    await onChanged()
  }

  const bundled = provider === 'flow2api' && mode === 'bundled'
  return (
    <div className="flex flex-col gap-3">
      {provider === 'flow2api' && (
        <Field label="Jalankan Flow2API">
          <Select
            label="Jalankan Flow2API"
            value={mode}
            onChange={setMode}
            options={[
              { value: 'bundled', label: 'Bawaan aplikasi (disarankan)' },
              { value: 'external', label: 'Server Flow2API milik sendiri' }
            ]}
          />
        </Field>
      )}
      <div className="flex items-end gap-2">
        {isEndpoint && !bundled && (
          <Field label="Alamat endpoint (base URL)" className="flex-[1.3]">
            <input
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              onKeyDown={onEnter}
              placeholder={provider === 'flow2api' ? 'http://localhost:8000' : 'http://localhost:11434/v1'}
              spellCheck={false}
              className={cx(inputCls, 'h-11 font-mono text-sm')}
            />
          </Field>
        )}
        {bundled && (
          <Field label="Kredensial" className="flex-[1.3]">
            <input
              readOnly
              value="Diurus otomatis oleh De Story"
              aria-label="Kredensial Flow2API bawaan"
              className={cx(inputCls, 'h-11 cursor-default bg-paper italic text-muted text-sm')}
            />
          </Field>
        )}
        <Field label={isEndpoint && !bundled ? 'API key (opsional)' : label} className="flex-1">
          <div className="relative">
            {bundled ? (
              <input
                readOnly
                value="Flow2API bawaan aktif"
                aria-label="Flow2API bawaan aktif"
                className={cx(inputCls, 'h-11 cursor-default bg-paper pr-11 italic text-muted text-sm')}
              />
            ) : showStoredKey ? (
              <input
                readOnly
                value={revealed ?? preview ?? ''}
                aria-label={`${label} tersimpan`}
                className={cx(inputCls, 'h-11 cursor-default bg-paper pr-11 font-mono text-sm')}
              />
            ) : showStoredNoKey ? (
              <input
                readOnly
                value="Tanpa autentikasi (publik)"
                aria-label={`${label} publik`}
                className={cx(inputCls, 'h-11 cursor-default bg-paper pr-11 italic text-muted text-sm')}
              />
            ) : (
              <input
                type={showTyped ? 'text' : 'password'}
                autoComplete="off"
                spellCheck={false}
                autoFocus={configured}
                value={key}
                onChange={(e) => setKey(e.target.value)}
                onKeyDown={onEnter}
                placeholder={
                  isEndpoint
                    ? preview
                      ? `Tersimpan (${preview}) · isi untuk mengganti`
                      : 'Kosongkan jika tanpa autentikasi'
                    : placeholder
                }
                className={cx(inputCls, 'h-11 pr-11')}
              />
            )}
            {!bundled && (showStoredKey || key.length > 0) && (
              <button
                type="button"
                aria-label={visible ? 'Sembunyikan kunci' : 'Tampilkan kunci'}
                title={visible ? 'Sembunyikan kunci' : 'Tampilkan kunci'}
                onClick={() => void toggleEye()}
                className="absolute right-1.5 top-1/2 flex size-8 -translate-y-1/2 items-center justify-center rounded-lg text-muted hover:bg-sand hover:text-ink"
              >
                {visible ? <EyeOff className="size-[17px]" /> : <Eye className="size-[17px]" />}
              </button>
            )}
          </div>
        </Field>
        {!bundled && (showStoredKey || showStoredNoKey) ? (
          <Button
            className="h-11"
            onClick={() => {
              setEditing(true)
              setRevealed(null)
            }}
          >
            Ganti
          </Button>
        ) : (
          <Button variant="accent" className="h-11" disabled={!canSave} loading={busy === 'save'} onClick={() => void save()}>
            {bundled ? 'Simpan' : 'Simpan dan tes'}
          </Button>
        )}
        {editing && configured && (
          <Button
            variant="ghost"
            className="h-11"
            onClick={() => {
              setEditing(false)
              setKey('')
              setUrl(customUrl ?? (provider === 'flow2api' ? 'http://localhost:8000' : ''))
            }}
          >
            Batal
          </Button>
        )}
        {configured && (
          <>
            <Button className="h-11" loading={busy === 'test'} onClick={() => void test()}>
              Tes
            </Button>
            <IconButton label={`Hapus ${name}`} size={44} className="text-bad-ink" onClick={() => void clear()}>
              <Trash2 className="size-[18px]" />
            </IconButton>
          </>
        )}
      </div>
      {unsaved && <p className="-mt-1 text-[13px] font-semibold text-accent-dark">Kunci belum tersimpan. Tekan Enter atau klik Simpan dan tes.</p>}
      {hint && <p className="-mt-1 text-[13px] text-ink-2">{hint}</p>}
      <div className="flex items-center gap-2 text-[13px] text-ink-2">
        <span className={cx('flex-1', configured && status?.lastOk === false && 'text-bad-ink')}>
          {configured && status?.lastMessage
            ? `${status.lastMessage}${status.checkedAt && !status.unreadable ? ` · dicek ${relativeTime(status.checkedAt)}` : ''}`
            : help}
        </span>
        {link && (
          <button
            type="button"
            onClick={() => void window.api.app.openExternal(link)}
            className="inline-flex shrink-0 items-center gap-1 font-semibold text-accent-dark hover:underline"
          >
            {linkLabel}
            <ExternalLink className="size-3.5" />
          </button>
        )}
      </div>
    </div>
  )
}

function GeminiKeyForm({
  status,
  help,
  link,
  linkLabel,
  onChanged
}: {
  status?: KeyStatus
  help: string
  link: string
  linkLabel: string
  onChanged: () => void | Promise<void>
}) {
  const { toast } = useApp()
  const configured = !!status?.configured
  const [editing, setEditing] = useState(!configured)
  const [keys, setKeys] = useState('')
  const [newKeys, setNewKeys] = useState('')
  const [busy, setBusy] = useState<'load' | 'save' | 'test' | 'add' | 'clear' | null>(null)
  const [results, setResults] = useState<GeminiKeyTestResult[] | null>(null)

  // Both Gemini forms share one pool, so a refreshed status makes any revealed replace draft stale.
  useEffect(() => {
    setEditing(!configured)
    setKeys('')
  }, [configured, status?.keyCount, status?.preview])

  const revealForEditing = async (): Promise<void> => {
    setBusy('load')
    try {
      setKeys((await window.api.settings.revealKey('gemini')) ?? '')
      setEditing(true)
      setResults(null)
    } catch (e) {
      toast('error', errorText(e))
    } finally {
      setBusy(null)
    }
  }

  const save = async (): Promise<void> => {
    const entries = keys.split(/\r?\n/).map((key) => key.trim()).filter(Boolean)
    setBusy('save')
    try {
      const checked = await window.api.settings.setGeminiKeys(entries)
      setResults(checked)
      const okCount = checked.filter((result) => result.ok).length
      toast(okCount ? 'success' : 'error', `${okCount} dari ${checked.length} API key Gemini terhubung`)
      setEditing(false)
      setKeys('')
      await onChanged()
    } catch (e) {
      toast('error', errorText(e))
    } finally {
      setBusy(null)
    }
  }

  const test = async (): Promise<void> => {
    setBusy('test')
    try {
      const checked = await window.api.settings.testGeminiKeys()
      setResults(checked)
      const okCount = checked.filter((result) => result.ok).length
      toast(okCount ? 'success' : 'error', `${okCount} dari ${checked.length} API key Gemini terhubung`)
      await onChanged()
    } catch (e) {
      toast('error', errorText(e))
    } finally {
      setBusy(null)
    }
  }

  // Appends new keys to the existing pool without revealing or replacing what is already stored.
  const append = async (): Promise<void> => {
    const entries = newKeys.split(/\r?\n/).map((key) => key.trim()).filter(Boolean)
    if (!entries.length) return
    setBusy('add')
    try {
      const checked = await window.api.settings.addGeminiKeys(entries)
      setResults(checked)
      const okCount = checked.filter((result) => result.ok).length
      toast(okCount ? 'success' : 'error', `${okCount} dari ${checked.length} API key Gemini baru terhubung`)
      setNewKeys('')
      await onChanged()
    } catch (e) {
      toast('error', errorText(e))
    } finally {
      setBusy(null)
    }
  }

  const clear = async (): Promise<void> => {
    const ok = await confirmDialog({
      title: 'Hapus semua API key Gemini?',
      body: 'Semua kunci Gemini yang tersimpan akan dihapus dari komputer ini. Kamu bisa menambahkannya lagi kapan saja.',
      confirm: 'Hapus semua',
      danger: true
    })
    if (!ok) return
    setBusy('clear')
    try {
      await window.api.settings.clearKey('gemini')
      setKeys('')
      setResults(null)
      await onChanged()
    } catch (e) {
      toast('error', errorText(e))
    } finally {
      setBusy(null)
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {editing ? (
        <Field label="API key Gemini · satu kunci per baris">
          <textarea
            value={keys}
            onChange={(e) => setKeys(e.target.value)}
            placeholder={'Tempel API key Google AI Studio di sini, satu per baris'}
            autoComplete="off"
            spellCheck={false}
            disabled={!!busy}
            rows={Math.min(6, Math.max(3, keys.split('\n').length))}
            className={cx(inputCls, 'min-h-24 resize-y py-2.5 font-mono text-sm')}
          />
        </Field>
      ) : (
        <div className="flex items-center gap-3">
          <div className="flex min-h-11 flex-1 items-center gap-2 rounded-xl border border-line bg-paper px-3 text-sm">
            <KeyRound className="size-4 shrink-0 text-muted" />
            <span className="font-mono">{status?.preview ?? '••••••••••'}</span>
            <span className="text-ink-2">{status?.keyCount ?? 0} kunci tersimpan</span>
          </div>
          <Button className="h-11" disabled={!!busy} loading={busy === 'load'} onClick={() => void revealForEditing()}>
            Kelola kunci
          </Button>
        </div>
      )}

      {editing ? (
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="accent" className="h-11" disabled={!keys.trim() || !!busy} loading={busy === 'save'} onClick={() => void save()}>
            Simpan dan tes semua
          </Button>
          {configured && (
            <Button className="h-11" disabled={!!busy} onClick={() => { setEditing(false); setKeys('') }}>
              Batal
            </Button>
          )}
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <Button className="h-11" disabled={!!busy} loading={busy === 'test'} onClick={() => void test()}>
            Tes semua
          </Button>
          <IconButton label="Hapus semua API key Gemini" size={44} className="text-bad-ink" disabled={!!busy} onClick={() => void clear()}>
            <Trash2 className="size-[18px]" />
          </IconButton>
        </div>
      )}

      {/* Always available while reading the stored pool: appends keys without revealing or replacing what is already stored. */}
      {!editing && (
        <div className="flex flex-col gap-2 border-t border-dashed border-line pt-3">
          <Field label="Tambah kunci baru · satu kunci per baris">
            <textarea
              value={newKeys}
              onChange={(e) => setNewKeys(e.target.value)}
              placeholder={'Tempel satu atau beberapa API key baru, satu per baris'}
              autoComplete="off"
              spellCheck={false}
              disabled={!!busy}
              rows={Math.min(4, Math.max(2, newKeys.split('\n').length))}
              className={cx(inputCls, 'min-h-16 resize-y py-2.5 font-mono text-sm')}
            />
          </Field>
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="accent" className="h-11" disabled={!newKeys.trim() || !!busy} loading={busy === 'add'} onClick={() => void append()}>
              Tambah
            </Button>
            <span className="text-[13px] text-ink-2">Kunci yang tersimpan tidak ditampilkan dan tidak diubah.</span>
          </div>
        </div>
      )}

      <p className="-mt-1 text-[13px] text-ink-2">{help}</p>
      <p className="-mt-2 text-[13px] text-ink-2">
        Untuk menambah beberapa kunci, masukkan satu per baris. Gunakan project Google berbeda untuk menambah kuota gabungan; batas tiap project tetap berlaku. Kunci disimpan terenkripsi di perangkat ini.
      </p>
      <div className="flex items-center gap-2 text-[13px] text-ink-2">
        <span className="flex-1">{configured && status?.lastMessage ? `${status.lastMessage}${status.checkedAt ? ` · dicek ${relativeTime(status.checkedAt)}` : ''}` : 'Kunci ini dipakai untuk Gemini LLM dan Gemini TTS.'}</span>
        <button type="button" onClick={() => void window.api.app.openExternal(link)} className="inline-flex shrink-0 items-center gap-1 font-semibold text-accent-dark hover:underline">
          {linkLabel}<ExternalLink className="size-3.5" />
        </button>
      </div>
      {results && results.length > 0 && (
        <ul className="flex flex-col gap-1.5 border-t border-dashed border-line pt-3 text-[13px]">
          {results.map((result) => (
            <li key={`${result.index}-${result.preview}`} className="flex items-start gap-2">
              {result.ok ? <Check className="mt-0.5 size-4 shrink-0 text-ok-ink" /> : <span className="mt-0.5 size-4 shrink-0 text-center text-bad-ink">!</span>}
              <span className="font-mono text-ink-2">#{result.index} {result.preview}</span>
              <span className={result.ok ? 'text-ink-2' : 'text-bad-ink'}>{result.message ?? (result.ok ? 'Terhubung' : 'Gagal')}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

function Card({ children }: { children: ReactNode }) {
  return <section className="flex flex-col gap-4 rounded-2xl border border-line bg-surface px-5 py-[18px]">{children}</section>
}

function SectionTitle({ icon, title, sub }: { icon: ReactNode; title: string; sub: ReactNode }) {
  return (
    <div className="mt-5 flex items-start gap-3">
      <span className="mt-1 text-ink">{icon}</span>
      <div className="flex flex-col gap-1">
        <h2 className="font-display text-[22px] font-bold leading-tight">{title}</h2>
        <p className="text-sm text-ink-2">{sub}</p>
      </div>
    </div>
  )
}

function LlmSection({ settings, statusOf, onChanged, onSettings }: { settings: AppSettings; statusOf: (p: ApiProvider) => KeyStatus | undefined; onChanged: () => void; onSettings: (s: AppSettings) => void }) {
  const { toast } = useApp()
  const provider = settings.llmProvider
  const spec = LLM_PROVIDERS.find((p) => p.id === provider)!
  const status = statusOf(provider)
  const configured = !!status?.configured
  const [openPicker, setOpenPicker] = useState(false)
  const models = useModels(provider, configured && status?.lastOk !== false, provider === 'custom' ? settings.customBaseUrl : '')
  const model = settings.llmModels[provider]
  const selected = models.models.find((m) => m.id === model)

  const options: ProviderOption<LlmProvider>[] = LLM_PROVIDERS.map((p) => ({
    id: p.id,
    name: p.name,
    note: p.note,
    logo: p.id,
    status: statusOf(p.id)
  }))

  return (
    <>
      <SectionTitle
        icon={<Cpu className="size-5" />}
        title="Penyusun cerita (AI LLM)"
        sub="Pilih satu penyedia untuk menyusun klip, naskah, dan pemeran. Hanya penyedia yang dipilih yang dipakai."
      />
      <ProviderSelect
        value={provider}
        options={options}
        onChange={async (id) => {
          setOpenPicker(false)
          onSettings(await window.api.settings.set({ llmProvider: id }))
        }}
      />
      <Card key={provider}>
        {provider === 'gemini' ? (
          <GeminiKeyForm
            status={status}
            help="Rotasi round-robin aktif: permintaan Gemini memakai kunci bergiliran."
            link="https://aistudio.google.com/apikey"
            linkLabel="Buat kunci di Google AI Studio"
            onChanged={async () => { await onChanged(); void models.refresh() }}
          />
        ) : (
          <KeyForm
            provider={provider}
            name={spec.name}
            status={status}
            help={provider === 'custom' ? 'Contoh: http://localhost:11434/v1 untuk Ollama, http://localhost:1234/v1 untuk LM Studio, https://api.x.ai/v1 untuk Grok.' : 'Setelah kunci disimpan, daftar model diambil langsung dari penyedianya.'}
            link={spec.link}
            linkLabel={spec.linkLabel}
            customUrl={settings.customBaseUrl}
            onChanged={onChanged}
            onSaved={async (r) => {
              if (!r.ok) return
              onSettings(await window.api.settings.get())
              if (configured) void models.refresh()
              if (!model) setOpenPicker(true)
            }}
          />
        )}
        {configured && status?.lastOk !== false && (
          <div className="flex flex-col gap-1.5 border-t border-dashed border-line pt-4">
            <span className="text-sm font-semibold">Model untuk menyusun cerita</span>
            <ModelPicker
              value={model}
              models={models.models}
              loading={models.loading}
              error={models.error}
              fetchedAt={models.fetchedAt}
              onChange={async (id) => {
                setOpenPicker(false)
                onSettings(await window.api.settings.set({ llmModels: { ...settings.llmModels, [provider]: id } }))
                toast('success', `Cerita akan disusun oleh ${spec.name} · ${id}`)
              }}
              onRefresh={() => void models.refresh()}
              allowCustom={provider === 'custom'}
              defaultOpen={openPicker}
              placeholder="Pilih model dari daftar"
            />
            {selected?.description && <p className="line-clamp-2 text-[13px] text-muted">{selected.description}</p>}
            {selected && provider === 'openrouter' && !selected.tags.includes('JSON') && (
              <p className="text-[13px] text-sun-ink">
                Model ini tidak mendukung output JSON terstruktur. Aplikasi tetap mencoba, tapi model berlabel JSON biasanya lebih stabil.
              </p>
            )}
          </div>
        )}
      </Card>
    </>
  )
}

function TtsSection({ settings, statusOf, onChanged, onSettings }: { settings: AppSettings; statusOf: (p: ApiProvider) => KeyStatus | undefined; onChanged: () => void; onSettings: (s: AppSettings) => void }) {
  const provider = settings.defaultTtsProvider
  const status = statusOf(provider)
  const ready = !!status?.configured && status.lastOk !== false
  const models = useModels(provider === 'gemini' ? 'gemini-tts' : 'elevenlabs', ready)
  const options: ProviderOption<TtsProvider>[] = [
    {
      id: 'gemini',
      name: 'Gemini TTS',
      note: 'Suara Google Gemini, 30 pilihan suara, mendukung bahasa Indonesia.',
      logo: 'gemini',
      status: statusOf('gemini')
    },
    {
      id: 'elevenlabs',
      name: 'ElevenLabs',
      note: 'Suara sangat natural dengan waktu kata yang presisi untuk caption karaoke.',
      logo: 'elevenlabs',
      status: statusOf('elevenlabs')
    }
  ]
  const value = provider === 'gemini' ? settings.geminiTtsModel : settings.elevenModel
  return (
    <>
      <SectionTitle
        icon={<Mic className="size-5" />}
        title="Suara narator (TTS)"
        sub="Penyedia suara bawaan untuk proyek baru. Setiap proyek tetap bisa memilih sendiri di langkah Ide cerita."
      />
      <ProviderSelect value={provider} options={options} onChange={async (id) => onSettings(await window.api.settings.set({ defaultTtsProvider: id }))} />
      <Card key={provider}>
        {provider === 'gemini' ? (
          <GeminiKeyForm
            status={status}
            help="Kunci Gemini yang sama dipakai juga untuk penyusun cerita."
            link="https://aistudio.google.com/apikey"
            linkLabel="Buat kunci di Google AI Studio"
            onChanged={async () => { await onChanged(); void models.refresh() }}
          />
        ) : (
          <KeyForm
            provider="elevenlabs"
            name="ElevenLabs"
            status={status}
            help="Kunci API dari akun ElevenLabs."
            link="https://elevenlabs.io/app/settings/api-keys"
            linkLabel="Buat kunci di ElevenLabs"
            onChanged={onChanged}
            onSaved={(r) => r.ok && void models.refresh()}
          />
        )}
        {ready && (
          <div className="flex flex-col gap-1.5 border-t border-dashed border-line pt-4">
            <span className="text-sm font-semibold">Model suara</span>
            <ModelPicker
              value={value}
              models={models.models}
              loading={models.loading}
              error={models.error}
              fetchedAt={models.fetchedAt}
              onChange={async (id) => onSettings(await window.api.settings.set(provider === 'gemini' ? { geminiTtsModel: id } : { elevenModel: id }))}
              onRefresh={() => void models.refresh()}
            />
          </div>
        )}
      </Card>
    </>
  )
}

function HiggsfieldSection({ status, onChanged }: { status?: KeyStatus; onChanged: () => void }) {
  return (
    <>
      <SectionTitle icon={<ImageIcon className="size-5" />} title="Gambar dan video" sub="Higgsfield membuat gambar klip, lembar karakter, dan video AI." />
      <Card>
        <div className="flex items-center gap-3">
          <ProviderLogo id="higgsfield" />
          <div className="flex flex-1 flex-col">
            <span className="text-base font-bold">Higgsfield</span>
            <span className="text-[13px] text-ink-2">GPT Image 2.5, Kling, Seedance, dan model lain dari akun Higgsfield kamu</span>
          </div>
          <StatusBadge status={status} />
        </div>
        <KeyForm
          provider="higgsfield"
          name="Higgsfield"
          status={status}
          placeholder="Tempel hasil tombol Copy API key"
          hint={
            <>
              Di console Higgsfield, buka API Keys lalu klik <strong className="text-ink">Copy API key</strong> dan tempel di sini. Formatnya{' '}
              <code className="rounded bg-sand px-1 font-mono text-xs">KEY_ID:KEY_SECRET</code>. Kalau kamu memegang Key ID dan Secret terpisah, gabungkan
              dengan titik dua.
            </>
          }
          help="Kunci dicek dengan permintaan estimasi biaya, tanpa memakai kredit."
          link="https://higgsfield.ai/s/higgsfield-api-yt-bangtutorial-lPMrHK"
          linkLabel="Buka console Higgsfield"
          onChanged={onChanged}
        />
        <p className="flex items-start gap-2.5 border-t border-dashed border-line pt-4 text-[13px] leading-relaxed text-ink-2">
          <Film className="mt-0.5 size-4 shrink-0" />
          <span>
            Semua {HF_IMAGE_MODELS.length} model gambar dan {HF_VIDEO_MODELS.length} model video Higgsfield bisa dipakai. Modelnya dipilih per proyek
            di langkah <strong className="text-ink">Ide cerita</strong>, bagian Model AI.
          </span>
        </p>
      </Card>
    </>
  )
}

function Flow2ApiSection({
  settings,
  status,
  onChanged,
  onSettings
}: {
  settings: AppSettings
  status?: KeyStatus
  onChanged: () => void
  onSettings: (s: AppSettings) => void
}) {
  return (
    <>
      <SectionTitle
        icon={<Sparkles className="size-5" />}
        title="Google Flow (Flow2API)"
        sub="Penyedia alternatif mandiri untuk generate gambar dan video AI gratis melalui gateway flow2api."
      />
      <Card>
        <div className="flex items-center gap-3">
          <ProviderLogo id="flow2api" />
          <div className="flex flex-1 flex-col">
            <span className="text-base font-bold">Google Flow via Flow2API</span>
            <span className="text-[13px] text-ink-2">
              Nano Banana 2.1 (Flash), Banana Pro, Google Veo 3.1 (Fast/Quality/Lite), Omni 1.1 Flash
            </span>
          </div>
          <StatusBadge status={status} />
        </div>
        <Flow2ApiBackendStatus mode={settings.flow2apiMode ?? 'bundled'} />
        <KeyForm
          provider="flow2api"
          name="Google Flow (Flow2API)"
          status={status}
          customUrl={settings.flow2apiBaseUrl || 'http://localhost:8000'}
          flow2apiMode={settings.flow2apiMode ?? 'bundled'}
          help="De Story bisa menjalankan Flow2API sendiri, atau tersambung ke server flow2api yang kamu jalankan."
          hint={
            <>
              Menjalankan repositori{' '}
              <a
                href="https://github.com/TheSmallHanCat/flow2api"
                target="_blank"
                rel="noreferrer"
                className="font-semibold text-accent-dark hover:underline"
              >
                TheSmallHanCat/flow2api
              </a>{' '}
              memungkinkan generate gambar Nano Banana dan video Google Veo 3.1 langsung tanpa memotong kredit Higgsfield.
            </>
          }
          link="https://github.com/TheSmallHanCat/flow2api"
          linkLabel="Buka panduan Flow2API di GitHub"
          onChanged={onChanged}
          onSaved={async (r) => {
            if (r.ok) {
              onSettings(await window.api.settings.get())
            }
          }}
        />
        <p className="flex items-start gap-2.5 border-t border-dashed border-line pt-4 text-[13px] leading-relaxed text-ink-2">
          <Film className="mt-0.5 size-4 shrink-0" />
          <span>
            {FLOW_IMAGE_MODELS.length} model gambar dan {FLOW_VIDEO_MODELS.length} model video Google Flow otomatis tersedia di pilihan Model AI proyek.
          </span>
        </p>
      </Card>
    </>
  )
}

/**
 * Health of the Flow2API process Bang Story runs itself. This is deliberately
 * separate from the token/captcha status badge above: a live backend can still
 * need a Google token before it can generate anything.
 */
function Flow2ApiBackendStatus({ mode }: { mode: 'bundled' | 'external' }) {
  const [status, setStatus] = useState<Flow2ApiStatus | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    if (mode === 'external') return
    let alive = true
    // Query on mount so an event fired before this screen opened is not missed.
    void window.api.flow2api.status().then((s) => {
      if (alive) setStatus(s)
    })
    const off = window.api.on.flow2api((s) => setStatus(s))
    return () => {
      alive = false
      off()
    }
  }, [mode])

  if (mode === 'external') {
    return (
      <p className="flex items-start gap-2.5 border-t border-dashed border-line pt-4 text-[13px] leading-relaxed text-ink-2">
        <Server className="mt-0.5 size-4 shrink-0" />
        <span>
          De Story tetap menjalankan Flow2API bawaannya di latar belakang, tetapi generate memakai server yang kamu tetapkan di
          atas.
        </span>
      </p>
    )
  }

  const tone: Record<string, string> = {
    ready: 'text-ok-ink',
    failed: 'text-bad-ink',
    stopped: 'text-muted'
  }
  const state = status?.state ?? 'starting'
  const label: Record<string, string> = {
    starting: 'Menyalakan Flow2API…',
    restarting: 'Mencoba menyalakan ulang Flow2API…',
    ready: 'Flow2API berjalan',
    failed: 'Flow2API gagal dijalankan',
    stopped: 'Flow2API berhenti'
  }

  return (
    <div className="flex flex-col gap-3 border-t border-dashed border-line pt-4">
      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-3">
          <Server className={cx('size-4 shrink-0', tone[state] ?? 'text-ink-2')} />
          <span className={cx('flex-1 text-[13px] font-semibold', tone[state] ?? 'text-ink-2')}>
            {label[state] ?? state}
          </span>
          <Button
            className="h-9"
            loading={busy}
            onClick={async () => {
              setBusy(true)
              try {
                setStatus(await window.api.flow2api.restart())
              } finally {
                setBusy(false)
              }
            }}
          >
            Nyalakan ulang
          </Button>
          <Button className="h-9" onClick={() => void window.api.flow2api.openFolder()}>
            Buka folder data
          </Button>
        </div>
        <p className="pl-7 text-[13px] leading-relaxed text-ink-2">
          {status?.message ??
            'Backend Flow2API bawaan sedang disiapkan. Token dan captcha Google tetap perlu diatur di server tersebut.'}
          {status?.baseUrl ? ` · ${status.baseUrl}` : ''}
        </p>
      </div>
      <Flow2ApiPanelCard baseUrl={status?.baseUrl ?? null} ready={state === 'ready'} />
      <Flow2ApiExtensionCard ready={state === 'ready'} />
    </div>
  )
}

/** A copyable value that never lands in a URL, log or screenshot by default. */
function SecretRow({
  label,
  value,
  onCopy
}: {
  label: string
  value: string | null
  onCopy: (text: string) => void
}) {
  const [shown, setShown] = useState(false)
  if (!value) {
    return (
      <div className="flex items-center gap-2 text-[13px] text-muted">
        <span className="w-24 shrink-0">{label}</span>
        <span>Belum tersedia — jalankan backend sekali lagi.</span>
      </div>
    )
  }
  return (
    <div className="flex items-center gap-2 text-[13px]">
      <span className="w-24 shrink-0 text-ink-2">{label}</span>
      <code className="min-w-0 flex-1 truncate rounded-lg bg-paper px-2.5 py-1.5 font-mono text-[12px]">
        {shown ? value : '••••••••••••••••'}
      </code>
      <IconButton label={shown ? 'Sembunyikan' : 'Tampilkan'} size={32} onClick={() => setShown(!shown)}>
        {shown ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
      </IconButton>
      <IconButton label={`Salin ${label}`} size={32} onClick={() => onCopy(value)}>
        <Copy className="size-4" />
      </IconButton>
    </div>
  )
}

/**
 * A shortcut into Flow2API's own admin panel, where every CAPTCHA method is
 * configured. Bang Story deliberately does not duplicate those forms: the
 * panel is the single source of truth for tokens, captcha providers and the
 * extension route keys.
 */
function Flow2ApiPanelCard({ baseUrl, ready }: { baseUrl: string | null; ready: boolean }) {
  const [info, setInfo] = useState<Flow2ApiPanelInfo | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [copied, setCopied] = useState<string | null>(null)

  // The password is read only when asked for, so it is not sitting in renderer
  // memory (or a React DevTools dump) for the whole session.
  const load = async (): Promise<void> => {
    setBusy(true)
    setError(null)
    try {
      setInfo(await window.api.flow2api.panelInfo())
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  useEffect(() => {
    // A restart changes the loopback port, so any cached credentials/URL are stale.
    setInfo(null)
    setError(null)
  }, [baseUrl])

  const copy = (text: string, what: string): void => {
    void window.api.app.copyText(text)
    setCopied(what)
    window.setTimeout(() => setCopied((c) => (c === what ? null : c)), 1800)
  }

  return (
    <div className="rounded-xl border border-line-2 bg-paper/60 p-3.5">
      <div className="flex items-center gap-3">
        <KeyRound className="size-4 shrink-0 text-ink-2" />
        <span className="flex-1 text-[13px] font-semibold">Panel admin Flow2API</span>
        {copied && <span className="text-[12px] text-ok-ink">Tersalin</span>}
        <Button className="h-9" disabled={!baseUrl} onClick={() => baseUrl && void window.api.app.openExternal(`${baseUrl}/login`)}>
          Buka panel admin
        </Button>
      </div>
      <p className="mt-2 text-[13px] leading-relaxed text-ink-2">
        Konfigurasi captcha — layanan solver berbayar, browser lokal, profil personal, remote browser, dan extension worker —
        dilakukan di panel ini. De Story hanya menyalakan servernya dan membukakan panelnya.
      </p>
      <div className="mt-3 flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <Button className="h-8" size="sm" loading={busy} disabled={!ready} onClick={() => void load()}>
            {info ? 'Muat ulang kredensial' : 'Tampilkan kredensial'}
          </Button>
          {info && (
            <Button className="h-8" size="sm" onClick={() => copy(`${info.username}\n${info.password}`, 'both')}>
              {copied === 'both' ? 'Tersalin' : 'Salin keduanya'}
            </Button>
          )}
        </div>
        {!ready && !info && (
          <p className="text-[12px] text-muted">Kredensial bisa ditampilkan setelah Flow2API selesai dinyalakan.</p>
        )}
        {error && <p className="text-[12px] text-bad-ink">{error}</p>}
        {info && (
          <>
            <SecretRow label="Nama pengguna" value={info.username} onCopy={(v) => copy(v, 'user')} />
            <SecretRow label="Sandi" value={info.password} onCopy={(v) => copy(v, 'pass')} />
            <p className="text-[12px] leading-relaxed text-muted">
              Tersimpan di folder data aplikasi. Sandi ini hanya untuk panel, terpisah dari API key server.
            </p>
          </>
        )}
      </div>
    </div>
  )
}

/**
 * The Chrome extension CAPTCHA worker, which generates Google Flow reCAPTCHA
 * tokens inside the user's own signed-in browser.
 *
 * The extension stores its credential once, so it can only be used when the
 * backend's address is stable — hence the opt-in fixed loopback port.
 */
function Flow2ApiExtensionCard({ ready }: { ready: boolean }) {
  const [info, setInfo] = useState<Flow2ApiExtensionInfo | null>(null)
  const [port, setPort] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [copied, setCopied] = useState<string | null>(null)

  const load = async (): Promise<void> => {
    try {
      setInfo(await window.api.flow2api.extensionInfo())
    } catch {
      setInfo(null)
    }
  }

  // Refresh once the backend is up: that is when the stable key file appears.
  useEffect(() => {
    void load()
  }, [ready])

  const apply = async (value: number): Promise<void> => {
    setBusy(true)
    setError(null)
    try {
      await window.api.flow2api.setFixedPort(value)
      setInfo(await window.api.flow2api.extensionInfo())
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  const copy = (text: string, what: string): void => {
    void window.api.app.copyText(text)
    setCopied(what)
    window.setTimeout(() => setCopied((c) => (c === what ? null : c)), 1800)
  }

  return (
    <div className="rounded-xl border border-line-2 bg-paper/60 p-3.5">
      <div className="flex items-center gap-3">
        <ExternalLink className="size-4 shrink-0 text-ink-2" />
        <span className="flex-1 text-[13px] font-semibold">Extension worker (opsional)</span>
        {info?.fixedPort ? <Badge tone="ok">Port tetap</Badge> : <Badge tone="draft">Port acak</Badge>}
      </div>
      <p className="mt-2 text-[13px] leading-relaxed text-ink-2">
        Membuat token reCAPTCHA Google Flow di dalam browser Chrome yang sudah login. Muat folder ekstensi sebagai{' '}
        <em>unpacked</em>, lalu isi opsi ekstensinya dengan nilai di bawah.
      </p>

      <div className="mt-3 flex flex-wrap items-center gap-2 border-t border-dashed border-line pt-3">
        <span className="text-[13px] text-ink-2">Port tetap loopback</span>
        <input
          className={cx(inputCls, 'h-9 w-24')}
          inputMode="numeric"
          placeholder={info?.fixedPort ? '' : '8765'}
          value={port}
          onChange={(e) => setPort(e.target.value.replace(/[^0-9]/g, ''))}
        />
        <Button
          className="h-9"
          size="sm"
          loading={busy}
          disabled={!port}
          onClick={() => {
            const value = Number(port)
            if (value < 1024 || value > 65535) {
              setError('Pakai port 1024–65535.')
              return
            }
            void apply(value)
          }}
        >
          Simpan dan nyalakan ulang
        </Button>
        {info?.fixedPort && (
          <Button className="h-9" size="sm" disabled={busy} onClick={() => void apply(0)}>
            Kembali ke port acak
          </Button>
        )}
      </div>
      <p className="mt-2 text-[12px] leading-relaxed text-muted">
        Hanya perlu kalau kamu memakai extension worker: ekstensi menyimpan alamatnya sekali, sedangkan port acak berubah setiap
        aplikasi dinyalakan. Tetap hanya di 127.0.0.1.
      </p>
      {error && <p className="mt-2 text-[12px] text-bad-ink">{error}</p>}

      <div className="mt-3 flex flex-col gap-2 border-t border-dashed border-line pt-3">
        <div className="flex items-center gap-2 text-[13px]">
          <span className="w-24 shrink-0 text-ink-2">WebSocket URL</span>
          <code className="min-w-0 flex-1 truncate rounded-lg bg-paper px-2.5 py-1.5 font-mono text-[12px]">
            {info?.wsUrl ?? '—'}
          </code>
          <IconButton label="Salin WebSocket URL" size={32} disabled={!info?.wsUrl} onClick={() => info && copy(info.wsUrl, 'url')}>
            <Copy className="size-4" />
          </IconButton>
        </div>
        <SecretRow label="Kunci" value={info?.key ?? null} onCopy={(v) => copy(v, 'key')} />
        {info?.fixedPort === false && info?.key && (
          <p className="text-[12px] leading-relaxed text-sun-ink">
            Alamat di atas berubah setiap aplikasi dinyalakan. Setel port tetap lebih dulu sebelum memasang ekstensinya.
          </p>
        )}

        <div className="mt-1 flex items-center gap-2">
          <Button className="h-9" size="sm" disabled={!info?.extensionDir} onClick={() => void window.api.flow2api.openExtensionFolder()}>
            Buka folder ekstensi
          </Button>
          {copied === 'url' || copied === 'key' ? <span className="text-[12px] text-ok-ink">Tersalin</span> : null}
        </div>
        <ol className="ml-4 list-decimal text-[12px] leading-relaxed text-muted">
          <li>Buka <code>chrome://extensions</code>, aktifkan Mode pengembang.</li>
          <li>Pilih <em>Load unpacked</em> dan arahkan ke folder di atas.</li>
          <li>Buka opsi ekstensi, isi WebSocket URL dan Kunci di atas, plus Route Key yang sama dengan token di panel admin.</li>
        </ol>
        <p className="text-[12px] leading-relaxed text-muted">
          Setelah itu pilih metode capture <strong>extension</strong> di panel admin. Status “terkonfigurasi” belum berarti
          captcha sudah diverifikasi.
        </p>
      </div>
    </div>
  )
}

export function Settings() {
  const { go, route } = useApp()
  const back = route.name === 'settings' && route.back ? route.back : ({ name: 'home' } as const)
  const [tab, setTab] = useState<Tab>('services')
  const [keys, setKeys] = useState<KeyStatus[]>([])
  const [settings, setSettings] = useState<AppSettings | null>(null)
  const [folder, setFolder] = useState('')

  const reload = async (): Promise<void> => {
    try {
      const [k, s] = await Promise.all([
        window.api.settings.keys(),
        window.api.settings.get()
      ])
      setKeys(k)
      setSettings(s)
    } catch (err) {
      console.error('Gagal mengambil pengaturan/kunci:', err)
    }
  }
  useEffect(() => {
    void reload()
    void window.api.exporter.defaultFolder().then(setFolder)
  }, [])

  const statusOf = (p: ApiProvider): KeyStatus | undefined => keys.find((k) => k.provider === p)

  const NAV: { id: Tab; label: string; icon: ReactNode }[] = [
    { id: 'services', label: 'Layanan AI dan kunci', icon: <KeyRound className="size-[18px]" /> },
    { id: 'general', label: 'Umum', icon: <SlidersHorizontal className="size-[18px]" /> },
    { id: 'about', label: 'Tentang aplikasi', icon: <Info className="size-[18px]" /> }
  ]

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-[68px] shrink-0 items-center gap-3.5 border-b border-line bg-surface px-8">
        <button type="button" onClick={() => go({ name: 'home' })}>
          <Logo />
        </button>
        <span className="text-xl text-line-3">/</span>
        <span className="font-semibold">Pengaturan</span>
        <div className="flex-1" />
        <Button icon={<ArrowLeft className="size-[18px]" />} onClick={() => go(back)}>
          {back.name === 'project' ? 'Kembali ke proyek' : 'Kembali ke beranda'}
        </Button>
      </header>
      <div className="grid min-h-0 flex-1 grid-cols-[280px_minmax(0,1fr)]">
        <nav aria-label="Bagian pengaturan" className="flex flex-col gap-1 border-r border-line pl-6 pr-4 pt-7">
          {NAV.map((n) => (
            <button
              key={n.id}
              type="button"
              aria-current={tab === n.id ? 'page' : undefined}
              onClick={() => setTab(n.id)}
              className={cx(
                'flex h-11 items-center gap-3 rounded-xl px-3.5 text-left text-[15px]',
                tab === n.id ? 'bg-ink font-semibold text-paper' : 'font-medium text-ink hover:bg-sand'
              )}
            >
              {n.icon}
              {n.label}
            </button>
          ))}
        </nav>
        <main className="scroll-thin overflow-y-auto">
          <div className="flex max-w-[900px] flex-col gap-3.5 px-14 pb-24 pt-8">
            {tab === 'services' && settings && (
              <>
                <div>
                  <h1 className="font-display text-[34px] font-bold tracking-[-0.015em]">Layanan AI dan kunci</h1>
                  <p className="mt-2 max-w-[700px] text-[15px] text-ink-2">
                    De Story memakai akun layanan milikmu sendiri. Kunci disimpan terenkripsi di komputer ini dan hanya dikirim langsung ke
                    layanan yang bersangkutan.
                  </p>
                </div>
                <LlmSection settings={settings} statusOf={statusOf} onChanged={reload} onSettings={setSettings} />
                <TtsSection settings={settings} statusOf={statusOf} onChanged={reload} onSettings={setSettings} />
                <HiggsfieldSection status={statusOf('higgsfield')} onChanged={reload} />
                <Flow2ApiSection settings={settings} status={statusOf('flow2api')} onChanged={reload} onSettings={setSettings} />
                <WhisperSection settings={settings} onSettings={setSettings} />
              </>
            )}

            {tab === 'general' && settings && (
              <>
                <div>
                  <h1 className="font-display text-[34px] font-bold tracking-[-0.015em]">Umum</h1>
                  <p className="mt-2 text-[15px] text-ink-2">Pilihan bawaan untuk proyek baru.</p>
                </div>
                <section className="flex flex-col gap-5 rounded-2xl border border-line bg-surface p-6">
                  <Field label="Bahasa default naskah dan suara">
                    <Select
                      label="Bahasa default naskah dan suara"
                      value={settings.defaultLanguage}
                      onChange={async (v) => setSettings(await window.api.settings.set({ defaultLanguage: v }))}
                      options={LANGUAGES.map((l) => ({ value: l.code, label: l.label }))}
                      className="max-w-[320px]"
                    />
                  </Field>
                  <Field label="Folder ekspor">
                    <div className="flex gap-2">
                      <div className="flex h-11 min-w-0 flex-1 items-center gap-2.5 rounded-xl border border-line bg-sand px-3.5 font-mono text-[13px]">
                        <FolderOpen className="size-[17px] shrink-0 text-ink-2" />
                        <span className="truncate">{folder}</span>
                      </div>
                      <Button
                        className="h-11"
                        onClick={async () => {
                          const f = await window.api.exporter.pickFolder()
                          if (f) setFolder(f)
                        }}
                      >
                        Ganti folder
                      </Button>
                    </div>
                  </Field>
                </section>
              </>
            )}

            {tab === 'about' && (
              <section className="flex flex-col gap-3.5 rounded-2xl border border-line bg-surface p-6 text-[15px] leading-relaxed text-ink-2">
                <div className="flex flex-col">
                  <h1 className="font-display text-[34px] font-bold tracking-[-0.015em] text-ink">De Story</h1>
                  <span className="text-sm font-medium text-muted">v1.1.0 · Modified by ikram.mf7</span>
                </div>
                <div className="rounded-xl border border-line bg-sand/60 p-4 text-[13.5px] leading-relaxed">
                  <div className="font-semibold text-ink">Kredit &amp; Atribusi Proyek Original:</div>
                  <p className="mt-1">
                    <strong>De Story</strong> versi 1.1.0 dimodifikasi dan dikembangkan oleh <strong>ikram.mf7</strong>.
                    Aplikasi ini dibangun berbasis proyek original <strong>Bang Story</strong> karya <strong>Bang Tutorial</strong>.
                  </p>
                </div>
                <p>Versi 1.1.0. Semua proyek, gambar, video, dan suara disimpan di komputer ini.</p>
                <p>
                  Aplikasi ini menyertakan FFmpeg (lisensi GPL) untuk merender video, whisper.cpp (lisensi MIT) untuk menyamakan caption dengan suara,
                  dan font Poppins (SIL Open Font License) untuk caption. Model Whisper Small (lisensi MIT) diunduh terpisah. File lisensinya ikut
                  terpasang di folder aplikasi.
                </p>
                <p>
                  Logo Gemini, OpenRouter, Groq, ElevenLabs, dan Higgsfield adalah merek milik pemiliknya masing-masing, dipakai hanya untuk menandai
                  layanan yang terhubung. Ikon penyedia berasal dari LobeHub Icons (lisensi MIT).
                </p>
              </section>
            )}
          </div>
        </main>
      </div>
    </div>
  )
}
