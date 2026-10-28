import { cx } from './cx'

/**
 * Brand marks for Higgsfield's model families (from LobeHub Icons, MIT; the marks belong to their owners).
 * Families without their own mark use their maker's: Alibaba for Wan, HappyHorse and Z-Image, and
 * Higgsfield for its in-house Soul and Cinema Studio.
 */
const FILES = import.meta.glob(['../assets/models/*.svg', '../assets/providers/higgsfield.png', '../assets/providers/gemini.svg'], { eager: true, import: 'default' }) as Record<string, string>
const file = (path: string): string | undefined => FILES[`../assets/${path}`]

const FAMILY_LOGO: Record<string, { src: string; bg: string; inset: number }> = {
  'Google Flow': { src: 'providers/gemini.svg', bg: '#FFFFFF', inset: 0.18 },
  Veo: { src: 'providers/gemini.svg', bg: '#FFFFFF', inset: 0.18 },
  'Nano Banana': { src: 'providers/gemini.svg', bg: '#FFFFFF', inset: 0.18 },
  'GPT Image': { src: 'models/openai.svg', bg: '#FFFFFF', inset: 0.2 },
  Soul: { src: 'providers/higgsfield.png', bg: '#D1FE17', inset: 0 },
  'Cinema Studio': { src: 'providers/higgsfield.png', bg: '#D1FE17', inset: 0 },
  Grok: { src: 'models/grok.svg', bg: '#FFFFFF', inset: 0.2 },
  Recraft: { src: 'models/recraft.svg', bg: '#FFFFFF', inset: 0.22 },
  Ideogram: { src: 'models/ideogram.svg', bg: '#FFFFFF', inset: 0.2 },
  Qwen: { src: 'models/qwen-color.svg', bg: '#FFFFFF', inset: 0.18 },
  'Z-Image': { src: 'models/alibaba-color.svg', bg: '#FFFFFF', inset: 0.16 },
  Kling: { src: 'models/kling-color.svg', bg: '#FFFFFF', inset: 0.18 },
  Seedance: { src: 'models/bytedance-color.svg', bg: '#FFFFFF', inset: 0.18 },
  Wan: { src: 'models/alibaba-color.svg', bg: '#FFFFFF', inset: 0.16 },
  HappyHorse: { src: 'models/alibaba-color.svg', bg: '#FFFFFF', inset: 0.16 },
  Hailuo: { src: 'models/hailuo-color.svg', bg: '#FFFFFF', inset: 0.18 },
  MiniMax: { src: 'models/minimax-color.svg', bg: '#FFFFFF', inset: 0.18 },
  PixVerse: { src: 'models/pixverse-color.svg', bg: '#FFFFFF', inset: 0.18 },
  LTX: { src: 'models/lightricks.svg', bg: '#FFFFFF', inset: 0.2 }
}

export function ModelLogo({ family, size = 30, className }: { family?: string; size?: number; className?: string }) {
  const look = family ? FAMILY_LOGO[family] : undefined
  const src = look && file(look.src)
  const inner = Math.round(size * (1 - (look?.inset ?? 0) * 2))
  return (
    <span
      className={cx('flex shrink-0 items-center justify-center overflow-hidden border border-line-2 font-bold text-ink-2', className)}
      style={{ width: size, height: size, borderRadius: Math.round(size * 0.26), background: look?.bg ?? '#F3EFE8', fontSize: Math.round(size * 0.4) }}
      aria-hidden="true"
    >
      {src ? <img src={src} alt="" draggable={false} style={{ width: inner, height: inner }} className="object-contain" /> : (family ?? '?').slice(0, 1)}
    </span>
  )
}
