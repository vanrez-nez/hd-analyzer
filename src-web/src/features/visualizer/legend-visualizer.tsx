import { fileColorForGroup, fileColorGroupForInput } from "@/file-colors"
import type { FileColorGroup } from "@/file-colors"
import type { ColorScheme } from "@/lib/use-system-color-scheme"
import type { VisualizerCellInput } from "./types"

type LegendVisualizerProps = {
  colorScheme: ColorScheme
  items: VisualizerCellInput[]
}

type LegendItem = {
  group: FileColorGroup
  label: string
  size: number
}

const LEGEND_LABELS: Record<FileColorGroup, string> = {
  folder: "Folders",
  file: "Files",
  audio: "Audio",
  video: "Video",
  document: "Documents",
}

export function LegendVisualizer({ colorScheme, items }: LegendVisualizerProps) {
  const legendItems = buildLegendItems(items)
  if (legendItems.length === 0) {
    return null
  }

  return (
    <div
      aria-label="Visualizer legend"
      className="pointer-events-none absolute right-2 top-2 flex min-w-32 select-none flex-col gap-1 rounded-md bg-background/80 px-2.5 py-2 text-right text-xs text-muted-foreground backdrop-blur-sm"
    >
      {legendItems.map((item) => (
        <div className="flex items-center justify-between gap-3" key={item.group}>
          <span className="min-w-0 flex-1 text-right">{item.label}</span>
          <span
            aria-hidden="true"
            className="size-2.5 shrink-0 rounded-[2px]"
            style={{ backgroundColor: fileColorForGroup(item.group, colorScheme) }}
          />
        </div>
      ))}
    </div>
  )
}

function buildLegendItems(items: VisualizerCellInput[]): LegendItem[] {
  const totals = new Map<FileColorGroup, number>()

  for (const item of items) {
    if (item.size <= 0) {
      continue
    }

    const group = fileColorGroupForInput(item)
    totals.set(group, (totals.get(group) ?? 0) + item.size)
  }

  return Array.from(totals.entries())
    .map(([group, size]) => ({
      group,
      label: LEGEND_LABELS[group],
      size,
    }))
    .sort((left, right) => right.size - left.size || left.label.localeCompare(right.label))
}
