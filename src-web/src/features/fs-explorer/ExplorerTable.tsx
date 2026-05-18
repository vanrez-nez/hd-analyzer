import { useMemo, useState } from "react"
import { ArrowDownIcon, ArrowUpIcon, FileIcon, Folder, HardDrive } from "lucide-react"

import { Spinner } from "@/components/ui/spinner"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { cn } from "@/lib/utils"
import type { DeleteSafetyClassification, DirectoryListingDto, DriveDto, PathNodeDto } from "./types"

type SortColumn = "name" | "size"
type SortDirection = "asc" | "desc"
type SortState = {
  column: SortColumn
  direction: SortDirection
}

type ExplorerTableProps = {
  className?: string
  volumes: DriveDto[]
  listing?: DirectoryListingDto
  loadingPath?: string
  onOpenVolume: (volume: DriveDto) => void
  onOpenNode: (node: PathNodeDto) => void
}

export function ExplorerTable({
  className,
  volumes,
  listing,
  loadingPath,
  onOpenVolume,
  onOpenNode,
}: ExplorerTableProps) {
  const [sort, setSort] = useState<SortState>({ column: "size", direction: "desc" })
  const rows = useMemo(() => {
    return listing ? sortNodes(listing.children, sort) : sortVolumes(volumes, sort)
  }, [listing, sort, volumes])

  const toggleSort = (column: SortColumn) => {
    setSort((current) => ({
      column,
      direction: current.column === column && current.direction === "desc" ? "asc" : "desc",
    }))
  }

  return (
    <div className={cn("flex min-h-0 flex-1 flex-col overflow-hidden rounded-md border", className)}>
      <Table className="block w-full">
        <TableHeader className="block">
          <TableRow className={tableRowClassName()}>
            <SortableHead column="name" sort={sort} onSort={toggleSort} className="w-full max-w-0">
              Name
            </SortableHead>
            <SortableHead column="size" sort={sort} onSort={toggleSort} className="whitespace-nowrap text-right" align="right">
              Size
            </SortableHead>
          </TableRow>
        </TableHeader>
      </Table>
      <div className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden overscroll-none">
        <Table className="block w-full">
          <TableBody className="block">
            {!listing
              ? (rows as DriveDto[]).map((volume) => (
                  <TableRow className={tableRowClassName("cursor-pointer select-none")} key={volume.id} onClick={() => onOpenVolume(volume)}>
                    <TableCell className="min-w-0 overflow-hidden whitespace-nowrap">
                      <span className="flex min-w-0 items-center gap-2 overflow-hidden">
                        <HardDrive data-icon="inline-start" />
                        <span className="truncate" title={volume.label}>
                          {volume.label}
                        </span>
                      </span>
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-right">{formatBytes(volume.usedSpace)}</TableCell>
                  </TableRow>
                ))
              : (rows as PathNodeDto[]).map((node) => (
                  <TableRow
                    className={tableRowClassName(node.kind === "directory" ? "cursor-pointer select-none" : "select-none")}
                    key={node.path}
                    title={node.deleteSafety?.reason}
                    onClick={() => {
                      if (node.kind === "directory") onOpenNode(node)
                    }}
                  >
                    <TableCell className="min-w-0 overflow-hidden whitespace-nowrap">
                      <span className="flex min-w-0 items-center gap-2 overflow-hidden">
                        {node.kind === "directory" ? (
                          <Folder
                            data-icon="inline-start"
                            className={cn("size-3.5", safetyIconClass(node.deleteSafety?.classification))}
                          />
                        ) : (
                          <FileIcon data-icon="inline-start" className="size-3.5 opacity-50" />
                        )}
                        <span
                          className={cn("truncate", node.kind === "directory" && safetyTextClass(node.deleteSafety?.classification))}
                          title={node.deleteSafety?.reason ? `${node.name} - ${node.deleteSafety.reason}` : node.name}
                        >
                          {node.name}
                        </span>
                      </span>
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-right">{renderNodeSize(node, loadingPath)}</TableCell>
                  </TableRow>
                ))}
          </TableBody>
        </Table>
      </div>
    </div>
  )
}

function tableRowClassName(className?: string) {
  return cn("grid grid-cols-[minmax(0,1fr)_max-content]", className)
}

function safetyTextClass(classification?: DeleteSafetyClassification) {
  switch (classification) {
    case "protected_system":
      return "text-destructive"
    case "not_deletable_now":
      return "text-chart-1"
    case "review_required":
      return "text-muted-foreground"
    case "safe_junk":
      return "text-chart-2"
    case "user_content":
    default:
      return undefined
  }
}

function safetyIconClass(classification?: DeleteSafetyClassification) {
  switch (classification) {
    case "protected_system":
      return "text-destructive"
    case "not_deletable_now":
      return "text-chart-1"
    case "review_required":
      return "text-muted-foreground"
    case "safe_junk":
      return "text-chart-2"
    case "user_content":
    default:
      return undefined
  }
}

type SortableHeadProps = {
  align?: "left" | "right"
  children: string
  className?: string
  column: SortColumn
  onSort: (column: SortColumn) => void
  sort: SortState
}

function SortableHead({ align = "left", children, className, column, onSort, sort }: SortableHeadProps) {
  const active = sort.column === column
  const Icon = sort.direction === "asc" ? ArrowUpIcon : ArrowDownIcon
  const handleKeyDown = (event: React.KeyboardEvent<HTMLTableCellElement>) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault()
      onSort(column)
    }
  }

  return (
    <TableHead
      aria-sort={active ? (sort.direction === "asc" ? "ascending" : "descending") : "none"}
      className={cn(
        "cursor-pointer select-none whitespace-nowrap bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        className,
      )}
      onClick={() => onSort(column)}
      onKeyDown={handleKeyDown}
      role="button"
      tabIndex={0}
    >
      <span className={cn("inline-flex h-8 items-center gap-1.5 rounded-md", align === "right" && "w-full justify-end")}>
        {children}
        {active ? <Icon data-icon="inline-end" className="size-3" /> : null}
      </span>
    </TableHead>
  )
}

function isWorking(state: string) {
  return state === "queued" || state === "working" || state === "partial" || state === "stale"
}

function sortVolumes(volumes: DriveDto[], sort: SortState) {
  return [...volumes].sort((left, right) => compareValues(volumeSortValue(left, sort.column), volumeSortValue(right, sort.column), sort))
}

function sortNodes(nodes: PathNodeDto[], sort: SortState) {
  return [...nodes].sort((left, right) => compareValues(nodeSortValue(left, sort.column), nodeSortValue(right, sort.column), sort))
}

function volumeSortValue(volume: DriveDto, column: SortColumn) {
  switch (column) {
    case "size":
      return { complete: true, value: volume.usedSpace }
    case "name":
      return volume.label
  }
}

function nodeSortValue(node: PathNodeDto, column: SortColumn) {
  switch (column) {
    case "size":
      return { complete: node.state === "complete", value: node.logicalSize }
    case "name":
      return node.name
  }
}

function compareValues(left: string | { complete: boolean; value: number }, right: string | { complete: boolean; value: number }, sort: SortState) {
  const result = typeof left === "string" && typeof right === "string" ? compareText(left, right) : compareSizeValues(left, right)
  return sort.direction === "asc" ? result : -result
}

function compareText(left: string, right: string) {
  return left.localeCompare(right, undefined, { numeric: true, sensitivity: "base" })
}

function compareSizeValues(
  left: string | { complete: boolean; value: number },
  right: string | { complete: boolean; value: number },
) {
  if (typeof left === "string" || typeof right === "string") {
    return compareText(String(left), String(right))
  }

  if (left.complete !== right.complete) {
    return left.complete ? 1 : -1
  }

  return left.value - right.value
}

function renderNodeSize(node: PathNodeDto, loadingPath?: string) {
  if (isWorking(node.state) || loadingPath === node.path) {
    if (node.logicalSize > 0) {
      return formatBytes(node.logicalSize)
    }

    return (
      <span className="inline-flex w-full justify-end">
        <Spinner />
      </span>
    )
  }

  if (node.state !== "complete") return ""
  return formatBytes(node.logicalSize)
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B"
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
