export function mmss(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000))
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

export function timecode(sec: number): string {
  const s = Math.max(0, sec)
  const m = Math.floor(s / 60)
  const r = s - m * 60
  return `${String(m).padStart(2, '0')}:${r.toFixed(1).padStart(4, '0')}`
}

export function secLabel(ms: number): string {
  return `${(ms / 1000).toFixed(1).replace('.', ',')} dtk`
}

export function relativeTime(ts: number): string {
  const diff = Date.now() - ts
  const min = Math.round(diff / 60000)
  if (min < 1) return 'baru saja'
  if (min < 60) return `${min} menit lalu`
  const h = Math.round(min / 60)
  if (h < 24) return `${h} jam lalu`
  const d = Math.round(h / 24)
  if (d === 1) return 'kemarin'
  if (d < 7) return `${d} hari lalu`
  const w = Math.round(d / 7)
  if (w < 5) return `${w} minggu lalu`
  return new Date(ts).toLocaleDateString('id-ID', { day: 'numeric', month: 'short', year: 'numeric' })
}

export function clipNumber(i: number): string {
  return String(i + 1).padStart(2, '0')
}

export function errorText(e: unknown): string {
  const msg = (e as Error)?.message ?? String(e)
  return msg.replace(/^Error invoking remote method '[^']+': (Error: )?/, '')
}

/** Higgsfield credits: whole numbers from 10 up, one decimal below. */
export function creditLabel(n: number): string {
  return (n >= 10 ? Math.round(n) : Math.round(n * 10) / 10).toLocaleString('id-ID')
}

export function usdLabel(n: number): string {
  return `US$${n.toLocaleString('id-ID', { minimumFractionDigits: 2, maximumFractionDigits: n < 1 ? 3 : 2 })}`
}
