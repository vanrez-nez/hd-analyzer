import { useEffect, useState } from "react"

export type ExplorerLoadingPhase = "opening" | "scanning"

export type ExplorerLoadingOverlayProps = {
  entriesProcessed: number
  forceVisible?: boolean
  infinite?: boolean
  operationKey: string
  phase: ExplorerLoadingPhase
  progress?: number
}

const LOADER_DISPLAY_DELAY_MS = 500
const PROGRESS_RING_SIZE = 180.5
const PROGRESS_RING_STROKE_WIDTH = 3
const PROGRESS_RING_RADIUS = (PROGRESS_RING_SIZE - PROGRESS_RING_STROKE_WIDTH) / 2
const PROGRESS_RING_INNER_RADIUS = PROGRESS_RING_RADIUS - PROGRESS_RING_STROKE_WIDTH / 2
const PROGRESS_RING_CIRCUMFERENCE = 2 * Math.PI * PROGRESS_RING_RADIUS

export function ExplorerLoadingOverlay({
  entriesProcessed,
  forceVisible = false,
  infinite = true,
  operationKey,
  phase,
  progress = 0,
}: ExplorerLoadingOverlayProps) {
  const [isVisible, setIsVisible] = useState(false)
  const phaseLabel = phase === "opening" ? "Opening" : "Scanning"
  const processedLabel = formatInteger(entriesProcessed)
  const clampedProgress = clamp(progress, 0, 100)
  const progressDashOffset = PROGRESS_RING_CIRCUMFERENCE - (clampedProgress / 100) * PROGRESS_RING_CIRCUMFERENCE

  useEffect(() => {
    if (forceVisible) {
      setIsVisible(true)
      return
    }

    setIsVisible(false)

    const timeoutId = window.setTimeout(() => {
      setIsVisible(true)
    }, LOADER_DISPLAY_DELAY_MS)

    return () => {
      window.clearTimeout(timeoutId)
    }
  }, [forceVisible, operationKey])

  if (!isVisible) {
    return null
  }

  return (
    <div
      className="absolute inset-0 flex items-center justify-center"
      aria-label={`${phaseLabel}: ${processedLabel} items processed`}
      aria-live="polite"
    >
      <div className="relative size-[180.5px] select-none">
        {infinite ? (
          <span className="fs-explorer-loader" aria-hidden="true" />
        ) : (
          <svg className="absolute inset-0 size-full -rotate-90" viewBox="0 0 180.5 180.5" aria-hidden="true">
            <circle cx="90.25" cy="90.25" r={PROGRESS_RING_INNER_RADIUS} fill="var(--background)" />
            <circle
              cx="90.25"
              cy="90.25"
              r={PROGRESS_RING_RADIUS}
              fill="none"
              stroke="color-mix(in oklch, var(--foreground) 20%, transparent)"
              strokeWidth={PROGRESS_RING_STROKE_WIDTH}
            />
            <circle
              cx="90.25"
              cy="90.25"
              r={PROGRESS_RING_RADIUS}
              fill="none"
              stroke="var(--foreground)"
              strokeDasharray={PROGRESS_RING_CIRCUMFERENCE}
              strokeDashoffset={progressDashOffset}
              strokeLinecap="round"
              strokeWidth={PROGRESS_RING_STROKE_WIDTH}
              className="transition-[stroke-dashoffset] duration-300 ease-out"
            />
          </svg>
        )}
        <div className="absolute inset-8 z-10 flex flex-col items-center justify-center gap-1 text-center">
          <span className="text-[0.6875rem] font-medium uppercase tracking-wide text-muted-foreground">
            Items Processed
          </span>
          <span className="max-w-full truncate text-[1rem] font-semibold tabular-nums">{processedLabel}</span>
        </div>
      </div>
    </div>
  )
}

function formatInteger(value: number) {
  return new Intl.NumberFormat("en-US").format(value)
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}
