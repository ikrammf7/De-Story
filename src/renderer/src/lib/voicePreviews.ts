/** Samples of Gemini 3.8 TTS voices (from Google AI Studio), bundled so previews play without an API call. */
const FILES = import.meta.glob('../assets/voices/*.mp3', { eager: true, import: 'default' }) as Record<string, string>

const BY_ID = new Map(Object.entries(FILES).map(([path, url]) => [path.replace(/^.*\/|\.mp3$/g, '').toLowerCase(), url]))

/** A bundled sample for a Gemini voice, matched by its id ("en-us-arlo") or display name ("Arlo"). */
export function geminiPreview(id: string, name?: string): string | null {
  return BY_ID.get(id.toLowerCase()) ?? (name ? BY_ID.get(`en-us-${name.toLowerCase()}`) : undefined) ?? null
}
