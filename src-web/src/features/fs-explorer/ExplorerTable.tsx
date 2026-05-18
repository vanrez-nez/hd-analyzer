import { useMemo, useState } from "react"
import { ArrowDownIcon, ArrowUpIcon, FileIcon, Folder, HardDrive } from "lucide-react"

import { Spinner } from "@/components/ui/spinner"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { cn } from "@/lib/utils"
import type { DeleteSafetyClassification, DirectoryListingDto, DriveDto, PathNodeDto } from "./types"

const NAME_MAX_LENGTH = 64

type SortColumn = "name" | "size"
type SortDirection = "asc" | "desc"
type SortState = {
  column: SortColumn
  direction: SortDirection
}

type ExplorerTableProps = {
  volumes: DriveDto[]
  listing?: DirectoryListingDto
  loadingPath?: string
  onOpenVolume: (volume: DriveDto) => void
  onOpenNode: (node: PathNodeDto) => void
}

export function ExplorerTable({
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
    <div className="min-h-0 flex-1 overflow-auto overscroll-none rounded-md border">
      <Table>
        <TableHeader>
          <TableRow>
            <SortableHead column="name" sort={sort} onSort={toggleSort}>
              Name
            </SortableHead>
            <SortableHead column="size" sort={sort} onSort={toggleSort} className="w-36">
              Size
            </SortableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {!listing
            ? (rows as DriveDto[]).map((volume) => (
                <TableRow className="cursor-pointer select-none" key={volume.id} onClick={() => onOpenVolume(volume)}>
                  <TableCell>
                    <span className="flex min-w-0 items-center gap-2">
                      <HardDrive data-icon="inline-start" />
                      <span title={volume.label}>{truncateMiddle(volume.label, NAME_MAX_LENGTH)}</span>
                    </span>
                  </TableCell>
                  <TableCell>{formatBytes(volume.usedSpace)}</TableCell>
                </TableRow>
              ))
            : (rows as PathNodeDto[]).map((node) => (
                <TableRow
                  className={node.kind === "directory" ? "cursor-pointer select-none" : "select-none"}
                  key={node.path}
                  title={node.deleteSafety?.reason}
                  onClick={() => {
                    if (node.kind === "directory") onOpenNode(node)
                  }}
                >
                  <TableCell>
                    <span className="flex min-w-0 items-center gap-2">
                      {node.kind === "directory" ? (
                        <Folder
                          data-icon="inline-start"
                          className={cn("size-3.5", safetyIconClass(node.deleteSafety?.classification))}
                        />
                      ) : (
                        <FileIcon data-icon="inline-start" className="size-3.5 opacity-50" />
                      )}
                      <span
                        className={node.kind === "directory" ? safetyTextClass(node.deleteSafety?.classification) : undefined}
                        title={node.deleteSafety?.reason ? `${node.name} - ${node.deleteSafety.reason}` : node.name}
                      >
                        {truncateMiddle(node.name, NAME_MAX_LENGTH)}
                      </span>
                    </span>
                  </TableCell>
                  <TableCell>{renderNodeSize(node, loadingPath)}</TableCell>
                </TableRow>
              ))}
        </TableBody>
      </Table>
    </div>
  )
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
  children: string
  className?: string
  column: SortColumn
  onSort: (column: SortColumn) => void
  sort: SortState
}

function SortableHead({ children, className, column, onSort, sort }: SortableHeadProps) {
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
        "sticky top-0 z-10 cursor-pointer select-none bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        className,
      )}
      onClick={() => onSort(column)}
      onKeyDown={handleKeyDown}
      role="button"
      tabIndex={0}
    >
      <span className="inline-flex h-8 items-center gap-1.5 rounded-md">
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
      <span className="inline-flex">
        <Spinner />
      </span>
    )
  }

  if (node.state !== "complete") return ""
  return formatBytes(node.logicalSize)
}

function truncateMiddle(value: string, maxLength: number) {
  if (value.length <= maxLength) {
    return value
  }

  if (maxLength <= 3) {
    return value.slice(0, Math.max(0, maxLength))
  }

  const available = maxLength - 3
  const headLength = Math.ceil(available / 2)
  const tailLength = Math.floor(available / 2)
  return `${value.slice(0, headLength)}...${value.slice(value.length - tailLength)}`
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
