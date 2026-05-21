import type { DriveDto } from "./types"

type VolumeDetailsProps = {
  volume: DriveDto
}

export function VolumeDetails({ volume }: VolumeDetailsProps) {
  const name = volumeNameOnly(volume.label, volume.mountPoint)
  const totalBytes = Math.max(1, volume.totalSpace)
  const freeBytes = Math.max(0, Math.min(volume.availableSpace, totalBytes))
  const usedBytes = Math.max(0, Math.min(volume.usedSpace, totalBytes))
  const freePercent = clamp((freeBytes / totalBytes) * 100, 0, 100)
  const usedPercent = clamp(100 - freePercent, 0, 100)

  return (
    <section className="flex min-h-14 select-none flex-col justify-center gap-2 overflow-hidden rounded-md border bg-muted/30 px-3 py-2">
      <div className="flex min-w-0 items-center justify-between gap-3">
        <div className="min-w-0 truncate text-sm font-medium" title={name}>
          {name}
        </div>
        <div className="shrink-0 whitespace-nowrap text-xs tabular-nums text-muted-foreground">
          {formatBytes(usedBytes)} used / {formatBytes(freeBytes)} free
        </div>
      </div>
      <div
        className="flex h-2 w-full overflow-hidden rounded-full bg-secondary"
        aria-label={`${formatBytes(usedBytes)} used, ${formatBytes(freeBytes)} free`}
        role="img"
      >
        <div className="h-full bg-primary" style={{ width: `${usedPercent}%` }} />
        <div className="h-full bg-muted" style={{ width: `${freePercent}%` }} />
      </div>
    </section>
  )
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "0 B"
  }

  const units = ["B", "KB", "MB", "GB", "TB"]
  let value = bytes
  let unit = 0
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000
    unit += 1
  }

  const maximumFractionDigits = unit === 0 ? 0 : 2
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits }).format(value)} ${units[unit]}`
}

function volumeNameOnly(label: string, rootPath: string) {
  const trimmed = label.trim()
  const suffixMatch = trimmed.match(/^(.*?)\s+\((.*)\)$/)
  if (suffixMatch?.[2] && normalizePath(suffixMatch[2]) === normalizePath(rootPath)) {
    return suffixMatch[1].trim() || lastPathPart(rootPath)
  }

  if (normalizePath(trimmed) === normalizePath(rootPath) || trimmed.startsWith("/")) {
    return lastPathPart(rootPath)
  }

  return trimmed || lastPathPart(rootPath)
}

function normalizePath(path: string) {
  const normalized = path.replaceAll("\\", "/").replace(/\/+$/, "")
  return normalized || "/"
}

function lastPathPart(path: string) {
  if (path === "/") {
    return "/"
  }

  return path.split("/").filter(Boolean).at(-1) ?? path
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}
