import { Server } from 'lucide-react'
import { cx } from './ui'

/**
 * Brand marks of the services the app connects to. Gemini, OpenRouter, Groq and ElevenLabs come from
 * LobeHub Icons (MIT); the Higgsfield mark is its official app icon. Logos belong to their owners.
 */
const FILES = import.meta.glob('../assets/providers/*.{svg,png}', { eager: true, import: 'default' }) as Record<string, string>
const file = (name: string): string => FILES[`../assets/providers/${name}`]

export type ProviderLogoId = 'gemini' | 'openrouter' | 'groq' | 'elevenlabs' | 'higgsfield' | 'custom' | 'flow2api'

const LOOK: Record<ProviderLogoId, { bg: string; src?: string; inset: number }> = {
  gemini: { bg: '#FFFFFF', src: 'gemini.svg', inset: 0.2 },
  openrouter: { bg: '#FFFFFF', src: 'openrouter.svg', inset: 0.2 },
  groq: { bg: '#F55036', src: 'groq.svg', inset: 0.2 },
  elevenlabs: { bg: '#000000', src: 'elevenlabs.svg', inset: 0.28 },
  higgsfield: { bg: '#D1FE17', src: 'higgsfield.png', inset: 0 },
  flow2api: { bg: '#FFFFFF', src: 'gemini.svg', inset: 0.18 },
  custom: { bg: '#F3EFE8', inset: 0.24 }
}

export function ProviderLogo({ id, size = 40, className }: { id: ProviderLogoId; size?: number; className?: string }) {
  const look = LOOK[id]
  const inner = Math.round(size * (1 - look.inset * 2))
  return (
    <span
      className={cx('flex shrink-0 items-center justify-center overflow-hidden border border-ink', className)}
      style={{ width: size, height: size, background: look.bg, borderRadius: Math.round(size * 0.26) }}
      aria-hidden="true"
    >
      {look.src ? (
        <img src={file(look.src)} alt="" draggable={false} style={{ width: inner, height: inner }} className="object-contain" />
      ) : (
        <Server style={{ width: inner, height: inner }} strokeWidth={1.8} className="text-ink" />
      )}
    </span>
  )
}
