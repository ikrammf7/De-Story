import type { CameraPresetId, MotionStrength, TransitionId } from './types'

/**
 * Camera motion shared by the live preview (CSS transforms) and the FFmpeg export (zoompan),
 * so what the user sees in the editor is what lands in the MP4.
 *
 * zoom >= 1. x and y are the camera centre offset inside the free space: -1 = far left/top, 1 = far right/bottom.
 */
export interface CameraFrame {
  zoom: number
  x: number
  y: number
}

export const CAMERA_PRESETS: { id: CameraPresetId; label: string }[] = [
  { id: 'zoomin', label: 'Zoom in' },
  { id: 'zoomout', label: 'Zoom out' },
  { id: 'panleft', label: 'Geser kiri' },
  { id: 'panright', label: 'Geser kanan' },
  { id: 'kenburns', label: 'Ken Burns' },
  { id: 'shake', label: 'Goyang halus' },
  { id: 'static', label: 'Diam' }
]

export const MOTION_STRENGTHS: { id: MotionStrength; label: string; amount: number }[] = [
  { id: 'halus', label: 'Halus', amount: 0.08 },
  { id: 'sedang', label: 'Sedang', amount: 0.15 },
  { id: 'kuat', label: 'Kuat', amount: 0.25 }
]

export const TRANSITIONS: { id: TransitionId; label: string; ffmpeg: string | null }[] = [
  { id: 'fade', label: 'Pudar', ffmpeg: 'fade' },
  { id: 'slideleft', label: 'Geser ke kiri', ffmpeg: 'slideleft' },
  { id: 'zoomin', label: 'Zoom tembus', ffmpeg: 'zoomin' },
  { id: 'cut', label: 'Potong langsung', ffmpeg: null }
]

export const TRANSITION_SEC = 0.4

export function strengthAmount(strength: MotionStrength): number {
  return MOTION_STRENGTHS.find((s) => s.id === strength)?.amount ?? 0.08
}

const smooth = (p: number): number => {
  const c = Math.min(1, Math.max(0, p))
  return c * c * (3 - 2 * c)
}

const lerp = (a: number, b: number, t: number): number => a + (b - a) * t

/** p is progress 0..1 through the clip, t is seconds since the clip started. */
export function cameraAt(preset: CameraPresetId, strength: MotionStrength, p: number, t: number): CameraFrame {
  const s = strengthAmount(strength)
  const e = smooth(p)
  switch (preset) {
    case 'zoomin':
      return { zoom: lerp(1, 1 + s, e), x: 0, y: 0 }
    case 'zoomout':
      return { zoom: lerp(1 + s, 1, e), x: 0, y: 0 }
    case 'panleft':
      return { zoom: 1 + s, x: lerp(0.9, -0.9, e), y: 0 }
    case 'panright':
      return { zoom: 1 + s, x: lerp(-0.9, 0.9, e), y: 0 }
    case 'kenburns':
      return { zoom: lerp(1 + s * 0.3, 1 + s, e), x: lerp(-0.5, 0.5, e), y: lerp(0.3, -0.3, e) }
    case 'shake':
      return {
        zoom: 1 + Math.max(0.04, s * 0.5),
        x: 0.35 * Math.sin(2 * Math.PI * 1.3 * t),
        y: 0.3 * Math.sin(2 * Math.PI * 1.7 * t + 1)
      }
    default:
      return { zoom: 1, x: 0, y: 0 }
  }
}

/** CSS transform for an image that exactly covers its frame. */
export function cameraCss(frame: CameraFrame): string {
  const free = ((frame.zoom - 1) / 2) * 100
  return `translate(${(-frame.x * free).toFixed(3)}%, ${(-frame.y * free).toFixed(3)}%) scale(${frame.zoom.toFixed(4)})`
}

/**
 * FFmpeg zoompan expressions for the same motion. `frames` is the clip length in output frames.
 * Uses the zoompan variables `on` (output frame index) and `iw`/`ih`.
 */
export function cameraZoompan(
  preset: CameraPresetId,
  strength: MotionStrength,
  frames: number,
  fps: number
): { z: string; x: string; y: string } {
  const s = strengthAmount(strength)
  const n = Math.max(1, frames - 1)
  const e = `(min(on/${n},1)*min(on/${n},1)*(3-2*min(on/${n},1)))`
  const t = `(on/${fps})`
  const lin = (a: number, b: number): string => `(${a}+(${b - a})*${e})`
  let z = '1'
  let x = '0'
  let y = '0'
  switch (preset) {
    case 'zoomin':
      z = lin(1, 1 + s)
      break
    case 'zoomout':
      z = lin(1 + s, 1)
      break
    case 'panleft':
      z = `${1 + s}`
      x = lin(0.9, -0.9)
      break
    case 'panright':
      z = `${1 + s}`
      x = lin(-0.9, 0.9)
      break
    case 'kenburns':
      z = lin(1 + s * 0.3, 1 + s)
      x = lin(-0.5, 0.5)
      y = lin(0.3, -0.3)
      break
    case 'shake':
      z = `${1 + Math.max(0.04, s * 0.5)}`
      x = `(0.35*sin(2*PI*1.3*${t}))`
      y = `(0.3*sin(2*PI*1.7*${t}+1))`
      break
    default:
      break
  }
  return {
    z,
    x: `(iw-iw/zoom)*(1+${x})/2`,
    y: `(ih-ih/zoom)*(1+${y})/2`
  }
}
