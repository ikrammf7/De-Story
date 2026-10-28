export function LogoMark({ size = 34 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 34 34" aria-hidden="true">
      <rect x="1" y="1" width="32" height="32" rx="9" fill="#C93A1B" stroke="#1F1D1A" strokeWidth="1.5" />
      <circle cx="17" cy="11" r="4.2" fill="#FFFFFF" stroke="#1F1D1A" strokeWidth="1.6" />
      <path
        d="M17 15.4v7.1M17 17.6l-5 3.4M17 17.6l5.2-4.2M17 22.5l-3.6 6M17 22.5l3.6 6"
        fill="none"
        stroke="#1F1D1A"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
    </svg>
  )
}

export function Logo() {
  return (
    <span className="flex items-center gap-2.5">
      <LogoMark />
      <span className="flex flex-col leading-none">
        <span className="font-display text-[21px] font-bold tracking-[-0.01em]">De Story</span>
        <span className="mt-[3px] self-end text-[10.5px] font-medium text-muted">by ikram.mf7</span>
      </span>
    </span>
  )
}
