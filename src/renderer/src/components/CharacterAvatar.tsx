import { cx } from './cx'

/**
 * A character's face for chips and lists. Character sheets are 16:9 with the front, three-quarter and
 * side views side by side, so this crops the head of the front (leftmost) view. Without a sheet it shows
 * the name's first letter.
 */
export function CharacterAvatar({
  name,
  sheetUrl,
  size = 28,
  busy,
  className
}: {
  name: string
  sheetUrl?: string | null
  size?: number
  /** Pulses while a sheet is being made. */
  busy?: boolean
  className?: string
}) {
  return (
    <span
      className={cx('relative flex shrink-0 items-center justify-center overflow-hidden rounded-full border border-ink bg-sand font-bold text-ink', className)}
      style={{ width: size, height: size, fontSize: Math.round(size * 0.42) }}
      aria-hidden="true"
    >
      {sheetUrl ? (
        // Measured on real sheets: the front view's head is centred about 22% across and 18% down. The crop
        // window is 20% of the sheet's width, starting 11.5% in and 2% down.
        <img src={sheetUrl} alt="" draggable={false} className="absolute max-w-none" style={{ width: size * 5, left: -size * 0.575, top: -size * 0.057 }} />
      ) : (
        name.trim().slice(0, 1).toUpperCase() || '?'
      )}
      {busy && <span className="absolute inset-0 animate-[pulse-soft_1.2s_ease-in-out_infinite] bg-sun/60" />}
    </span>
  )
}
