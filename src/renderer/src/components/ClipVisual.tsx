import { AlertCircle, Image as ImageIcon } from 'lucide-react'
import type { Asset, Clip, Job } from '@shared/types'
import { isRunning } from '../store/project'
import { Progress, cx } from './ui'

/** A clip's picture: its video (if chosen), image, or a placeholder, with the state of any running job on top. */
export function ClipVisual({
  clip,
  image,
  video,
  job,
  large,
  playVideo
}: {
  clip: Clip
  image?: Asset | null
  video?: Asset | null
  job?: Job | null
  large?: boolean
  playVideo?: boolean
}) {
  const running = isRunning(job)
  const failed = job?.status === 'failed' && !image
  const showVideo = clip.motionType === 'video' && video && playVideo
  return (
    <div className="absolute inset-0">
      {showVideo ? (
        <video src={video.url} poster={image?.url} className="size-full object-cover" autoPlay loop muted playsInline />
      ) : image ? (
        <img src={image.url} alt="" className="size-full object-cover" draggable={false} />
      ) : running ? null : (
        <div
          className={cx(
            'absolute inset-2 flex flex-col items-center justify-center gap-1.5 rounded-[10px] border-[1.5px] border-dashed text-muted',
            failed ? 'border-bad-ink/40 bg-bad-soft text-bad-ink' : 'border-line-3 bg-paper'
          )}
        >
          {failed ? <AlertCircle className={large ? 'size-7' : 'size-5'} /> : <ImageIcon className={large ? 'size-7' : 'size-5'} strokeWidth={1.6} />}
          <span className={large ? 'text-sm' : 'text-[13px]'}>{failed ? 'Gagal dibuat' : 'Belum ada gambar'}</span>
        </div>
      )}
      {running && (
        // Over an older picture, blur it so it reads as "being replaced"; over nothing, stay opaque.
        <div
          className={cx(
            'absolute inset-0 flex flex-col items-center justify-center gap-2',
            image || showVideo ? 'bg-paper/80 backdrop-blur-[3px]' : 'bg-paper'
          )}
        >
          <span className={cx('font-semibold text-sun-ink', large ? 'text-[15px]' : 'text-[13px]')}>
            {job!.status === 'queued' ? 'Menunggu antrean…' : `${job!.message ?? 'Memproses'}… ${Math.round(job!.progress * 100)}%`}
          </span>
          <Progress value={job!.progress} className="w-3/5" />
          {large && <span className="text-[13px] text-ink-2">Boleh pindah klip, prosesnya tetap jalan</span>}
        </div>
      )}
    </div>
  )
}
