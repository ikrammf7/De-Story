import { useEffect, useState } from 'react'

const cache = new Map<string, Promise<number[]>>()
let ctx: AudioContext | null = null

/** Normalised peak amplitude per bucket, decoded once per file. */
function peaks(url: string, buckets = 160): Promise<number[]> {
  const key = `${url}#${buckets}`
  if (!cache.has(key)) {
    cache.set(
      key,
      (async () => {
        const buf = await (await fetch(url)).arrayBuffer()
        ctx ??= new AudioContext()
        const audio = await ctx.decodeAudioData(buf)
        const data = audio.getChannelData(0)
        const size = Math.max(1, Math.floor(data.length / buckets))
        const out: number[] = []
        let max = 0
        for (let b = 0; b < buckets; b++) {
          let peak = 0
          for (let i = b * size; i < Math.min(data.length, (b + 1) * size); i += 16) peak = Math.max(peak, Math.abs(data[i]))
          out.push(peak)
          max = Math.max(max, peak)
        }
        return out.map((p) => (max ? p / max : 0))
      })().catch(() => [])
    )
  }
  return cache.get(key)!
}

export function Waveform({ url, className }: { url: string; className?: string }) {
  const [data, setData] = useState<number[]>([])
  useEffect(() => {
    let alive = true
    void peaks(url).then((p) => alive && setData(p))
    return () => {
      alive = false
    }
  }, [url])
  if (!data.length) return null
  const d = data.map((v, i) => `M${i + 0.5} ${50 - Math.max(3, v * 46)}V${50 + Math.max(3, v * 46)}`).join('')
  return (
    <svg viewBox={`0 0 ${data.length} 100`} preserveAspectRatio="none" className={className} aria-hidden="true">
      <path d={d} stroke="#8C7F6C" strokeWidth="1.5" strokeLinecap="round" fill="none" vectorEffect="non-scaling-stroke" />
    </svg>
  )
}
