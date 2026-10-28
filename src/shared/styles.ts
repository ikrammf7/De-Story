import styleCatalog from './visual-styles.json'

export type StyleCategory = '2d' | '3d' | 'realistis'

export interface VisualStyle {
  id: string
  name: string
  category: StyleCategory
  niche: string
  /** Tile colors used for the picker until a real sample image exists. */
  swatch: string
  ink: string
  /** Appended to every image prompt in the project. */
  prompt: string
  /** How characters should look on the character sheet. */
  characterHint: string
}

export const STYLE_CATEGORIES: { id: StyleCategory | 'all'; label: string }[] = [
  { id: 'all', label: 'Semua' },
  { id: '2d', label: '2D' },
  { id: '3d', label: '3D' },
  { id: 'realistis', label: 'Realistis' }
]

export const VISUAL_STYLES: VisualStyle[] = styleCatalog.map((style) => ({
  ...style,
  category: style.category as StyleCategory
}))

export function getStyle(id: string): VisualStyle {
  return VISUAL_STYLES.find((s) => s.id === id) ?? VISUAL_STYLES[0]
}
