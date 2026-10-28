/**
 * Inline vocal tags in narration, written the Gemini TTS way: English words in angle brackets, like
 * <short pause> or <chuckles>. They steer the voice and are never shown in captions.
 */
export const VOICE_TAGS = ['short pause', 'long pause', 'chuckles', 'laugh', 'sigh', 'gasp', 'whispers', 'breath']

const ANGLE = /<[^<>]{1,40}>/g
const SQUARE = /\[[^[\]]{1,40}\]/g

/** The words a listener hears: tags removed, spacing tidied. */
export function stripVoiceTags(text: string): string {
  return text.replace(ANGLE, ' ').replace(SQUARE, ' ').replace(/\s+/g, ' ').trim()
}

export function spokenWords(text: string): string[] {
  return stripVoiceTags(text).split(/\s+/).filter(Boolean)
}

/** Extra seconds the pause tags add to a line. */
export function tagPauseSec(text: string): number {
  const short = (text.match(/<short pause>/gi) ?? []).length
  const long = (text.match(/<long pause>/gi) ?? []).length
  return short * 0.4 + long * 1
}

/**
 * The same line for ElevenLabs: v3 models take tags in square brackets; older models only understand
 * <break> pauses, so other tags are dropped there.
 */
export function toElevenLabsText(text: string, model: string): string {
  if (/v3/i.test(model)) return text.replace(/<([^<>]{1,40})>/g, '[$1]')
  return text
    .replace(/<short pause>/gi, '<break time="0.4s" />')
    .replace(/<long pause>/gi, '<break time="1.0s" />')
    .replace(/<(?!break\b)[^<>]{1,40}>/gi, ' ')
    .replace(/\s{2,}/g, ' ')
}

/** True for word tokens that are only tag syntax (e.g. "[laughs]" or parts of a <break> tag). */
export function isTagToken(word: string): boolean {
  return /[<>[\]=]|^\/$/.test(word)
}
