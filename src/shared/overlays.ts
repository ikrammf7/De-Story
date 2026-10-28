import type { ImageOverlay, Overlay, TextOverlay, TextOverlayStyle } from './types'

/**
 * Logos, watermarks and text drawn over the video. The editor preview and the FFmpeg export both read
 * these helpers, so an overlay lands in the same spot in both.
 */

export type Anchor = Overlay['anchor']

/** Gap between a corner overlay and the frame edge, as a fraction of the frame width. */
export const OVERLAY_MARGIN = 0.035

export const TEXT_SIZES: { id: string; label: string; size: number }[] = [
  { id: 'kecil', label: 'Kecil', size: 0.045 },
  { id: 'sedang', label: 'Sedang', size: 0.065 },
  { id: 'besar', label: 'Besar', size: 0.09 }
]

export const TEXT_COLORS = ['#FFFFFF', '#1F1D1A', '#FFD43B', '#E4572E', '#4DABF7', '#51CF66']

export const TEXT_STYLES: { id: TextOverlayStyle; label: string }[] = [
  { id: 'box', label: 'Kotak' },
  { id: 'outline', label: 'Garis' },
  { id: 'plain', label: 'Polos' }
]

/** Where the anchor sits on the overlay itself: 0, 0.5 or 1 of its width (ax) and height (ay). Numpad layout. */
export function anchorOffset(anchor: Anchor): { ax: number; ay: number } {
  const col = (anchor - 1) % 3
  const row = Math.floor((anchor - 1) / 3)
  return { ax: col / 2, ay: row === 2 ? 0 : row === 1 ? 0.5 : 1 }
}

/** The frame point matching an anchor preset, with the same pixel margin from the side and the top or bottom. */
export function anchorPoint(anchor: Anchor, frameAspect: number): { x: number; y: number } {
  const { ax, ay } = anchorOffset(anchor)
  const mx = OVERLAY_MARGIN
  const my = OVERLAY_MARGIN * frameAspect
  return { x: ax === 0 ? mx : ax === 1 ? 1 - mx : 0.5, y: ay === 0 ? my : ay === 1 ? 1 - my : 0.5 }
}

export function overlayEnd(o: Overlay, total: number): number {
  return Math.min(o.end ?? total, total)
}

export function overlayVisible(o: Overlay, t: number, total: number): boolean {
  return t >= o.start && t < overlayEnd(o, total)
}

/** Dark ink for light text and white for dark text: used for boxes and outlines. */
export function contrastColor(hex: string): string {
  const h = hex.replace('#', '')
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255)
  return 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.55 ? '#1F1D1A' : '#FFFFFF'
}

/** Box padding, outline width and shadow offset, in pixels, for text of `fontPx`. */
export function textEffects(style: TextOverlayStyle, fontPx: number): { pad: number; outline: number; shadow: number } {
  return {
    pad: style === 'box' ? Math.round(fontPx * 0.28) : 0,
    outline: style === 'outline' ? Math.max(1, Math.round(fontPx * 0.08)) : 0,
    shadow: style === 'plain' ? Math.max(1, Math.round(fontPx * 0.05)) : 0
  }
}

export function newImageOverlay(assetId: string, frameAspect: number): ImageOverlay {
  return {
    id: crypto.randomUUID(),
    kind: 'image',
    assetId,
    anchor: 9,
    ...anchorPoint(9, frameAspect),
    width: frameAspect >= 1 ? 0.14 : 0.24,
    opacity: 0.9,
    start: 0,
    end: null
  }
}

export function newTextOverlay(start: number, end: number, frameAspect: number): TextOverlay {
  return {
    id: crypto.randomUUID(),
    kind: 'text',
    text: 'Teks baru',
    anchor: 8,
    ...anchorPoint(8, frameAspect),
    size: 0.065,
    color: '#FFFFFF',
    style: 'box',
    bold: true,
    start,
    end
  }
}
