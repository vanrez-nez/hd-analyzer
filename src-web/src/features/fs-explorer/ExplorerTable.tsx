import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  ArrowDownIcon,
  ArrowUpIcon,
  EyeIcon,
  FileIcon,
  Folder,
  FolderOpenIcon,
  HardDrive,
  Trash2Icon,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { openItemLocation, previewItem } from "@/api"
import { isToggleSelectionInput, rangeSelection, replaceSelection, toggleSelection } from "@/lib/selection"
import { cn } from "@/lib/utils"
import type { DeleteSafetyClassification, DirectoryListingDto, DriveDto, PathNodeDto } from "./types"
import type { MouseEvent } from "react"

type SortColumn = "name" | "size"
type SortDirection = "asc" | "desc"
type SortState = {
  column: SortColumn
  direction: SortDirection
}
type StatusItem = {
  canDeleteNow?: boolean
  fileSystem?: string
  id: string
  kind: "volume" | PathNodeDto["kind"]
  label: string
  mountPoint?: string
  path: string
  size: number
  totalSpace?: number
  availableSpace?: number
}

const ROW_ICON_CLASS = "size-3.5 shrink-0"
const EXPLORER_ROW_SELECTOR = "[data-explorer-row]"

type ExplorerTableProps = {
  className?: string
  volumes: DriveDto[]
  listing?: DirectoryListingDto
  loadingPath?: string
  selectedItemIds: string[]
  selectionAnchorId: string | null
  onOpenVolume: (volume: DriveDto) => void
  onOpenNode: (node: PathNodeDto) => void
  onSelectionAnchorChange: (itemId: string | null) => void
  onSelectionChange: (itemIds: string[]) => void
}

export function ExplorerTable({
  className,
  volumes,
  listing,
  loadingPath,
  selectedItemIds,
  selectionAnchorId,
  onOpenVolume,
  onOpenNode,
  onSelectionAnchorChange,
  onSelectionChange,
}: ExplorerTableProps) {
  const rowElementsRef = useRef(new Map<string, HTMLTableRowElement>())
  const [sort, setSort] = useState<SortState>({ column: "size", direction: "desc" })
  const rows = useMemo(() => {
    return listing ? sortNodes(listing.children, sort) : sortVolumes(volumes, sort)
  }, [listing, sort, volumes])
  const statusItems = useMemo(() => {
    return listing
      ? (rows as PathNodeDto[]).map(nodeToStatusItem)
      : (rows as DriveDto[]).map(volumeToStatusItem)
  }, [listing, rows])
  const visibleItemIds = useMemo(() => {
    return listing
      ? (rows as PathNodeDto[]).map((node) => node.path)
      : (rows as DriveDto[]).map((volume) => volume.id)
  }, [listing, rows])
  const selectedItemSet = useMemo(() => new Set(selectedItemIds), [selectedItemIds])
  const selectedStatusItems = useMemo(() => {
    return statusItems.filter((item) => selectedItemSet.has(item.id))
  }, [selectedItemSet, statusItems])
  const setRowElement = useCallback((itemId: string, element: HTMLTableRowElement | null) => {
    if (element) {
      rowElementsRef.current.set(itemId, element)
      return
    }

    rowElementsRef.current.delete(itemId)
  }, [])

  useEffect(() => {
    if (selectedItemIds.length === 0) {
      return
    }

    const firstVisibleSelectedItemId = visibleItemIds.find((itemId) => selectedItemSet.has(itemId))
    if (!firstVisibleSelectedItemId) {
      return
    }

    rowElementsRef.current.get(firstVisibleSelectedItemId)?.scrollIntoView({
      block: "nearest",
      inline: "nearest",
    })
  }, [selectedItemIds, selectedItemSet, visibleItemIds])

  const selectRow = (itemId: string, event: MouseEvent<HTMLTableRowElement>) => {
    if (!listing) {
      onSelectionChange(replaceSelection([itemId]))
      onSelectionAnchorChange(itemId)
      return
    }

    if (event.shiftKey) {
      onSelectionChange(rangeSelection(visibleItemIds, selectionAnchorId, itemId))
      if (!selectionAnchorId) {
        onSelectionAnchorChange(itemId)
      }
      return
    }

    if (isToggleSelectionInput(event)) {
      onSelectionChange(toggleSelection(selectedItemIds, [itemId]))
      onSelectionAnchorChange(itemId)
      return
    }

    onSelectionChange(replaceSelection([itemId]))
    onSelectionAnchorChange(itemId)
  }

  const clearSelectionOnEmptyListClick = (event: MouseEvent<HTMLDivElement>) => {
    if (event.target instanceof Element && event.target.closest(EXPLORER_ROW_SELECTOR)) {
      return
    }

    if (selectedItemIds.length === 0 && !selectionAnchorId) {
      return
    }

    onSelectionChange([])
    onSelectionAnchorChange(null)
  }

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
          <TableRow className={tableRowClassName("hover:bg-transparent")}>
            <SortableHead column="name" sort={sort} onSort={toggleSort} className="min-w-0">
              Name
            </SortableHead>
            <SortableHead column="size" sort={sort} onSort={toggleSort} className="whitespace-nowrap text-right" align="right">
              Size
            </SortableHead>
          </TableRow>
        </TableHeader>
      </Table>
      <div
        className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden overscroll-none"
        onClick={clearSelectionOnEmptyListClick}
      >
        <Table className="block w-full">
          <TableBody className="block">
            {!listing
              ? (rows as DriveDto[]).map((volume) => {
                  const selected = selectedItemSet.has(volume.id)

                  return (
                    <TableRow
                      aria-selected={selected}
                      className={selectableRowClassName("cursor-pointer select-none", selected)}
                      data-explorer-row
                      key={volume.id}
                      ref={(element) => setRowElement(volume.id, element)}
                      onClick={(event) => selectRow(volume.id, event)}
                      onDoubleClick={() => onOpenVolume(volume)}
                    >
                      <TableCell className="min-w-0 overflow-hidden whitespace-nowrap">
                        <span className="flex min-w-0 items-center gap-2 overflow-hidden">
                          <HardDrive data-icon="inline-start" className={ROW_ICON_CLASS} />
                          <span className="truncate" title={volume.label}>
                            {volume.label}
                          </span>
                        </span>
                      </TableCell>
                      <TableCell className="whitespace-nowrap text-right">{formatBytes(volume.usedSpace)}</TableCell>
                    </TableRow>
                  )
                })
              : (rows as PathNodeDto[]).map((node) => {
                  const selected = selectedItemSet.has(node.path)

                  return (
                    <TableRow
                      aria-selected={selected}
                      className={selectableRowClassName(
                        node.kind === "directory" ? "cursor-pointer select-none" : "select-none",
                        selected,
                      )}
                      data-explorer-row
                      key={node.path}
                      ref={(element) => setRowElement(node.path, element)}
                      title={node.deleteSafety?.reason}
                      onClick={(event) => selectRow(node.path, event)}
                      onDoubleClick={() => {
                        if (node.kind === "directory") onOpenNode(node)
                      }}
                    >
                      <TableCell className="min-w-0 overflow-hidden whitespace-nowrap">
                        <span className="flex min-w-0 items-center gap-2 overflow-hidden">
                          {node.kind === "directory" ? (
                            <Folder
                              data-icon="inline-start"
                              className={cn(ROW_ICON_CLASS, safetyIconClass(node.deleteSafety?.classification))}
                            />
                          ) : (
                            <FileIcon data-icon="inline-start" className={cn(ROW_ICON_CLASS, "opacity-50")} />
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
                  )
                })}
          </TableBody>
        </Table>
      </div>
      <ExplorerStatusBar listing={listing} rows={statusItems} selectedRows={selectedStatusItems} />
    </div>
  )
}

function ExplorerStatusBar({
  listing,
  rows,
  selectedRows,
}: {
  listing?: DirectoryListingDto
  rows: StatusItem[]
  selectedRows: StatusItem[]
}) {
  const selected = selectedRows[0]
  const isRootVolumes = !listing

  if (isRootVolumes) {
    return (
      <div className="flex min-h-9 items-center justify-between gap-3 border-t bg-muted/50 px-3 text-xs">
        {selected ? (
          <>
            <span className="min-w-0 truncate font-medium" title={selected.label}>
              {selected.label}
            </span>
            <span className="flex min-w-0 items-center justify-end gap-3 text-muted-foreground">
              <span className="whitespace-nowrap">Total {formatBytes(selected.totalSpace ?? 0)}</span>
              <span className="whitespace-nowrap">Free {formatBytes(selected.availableSpace ?? 0)}</span>
              <span className="max-w-24 truncate" title={selected.fileSystem}>
                {selected.fileSystem || "Unknown FS"}
              </span>
              <span className="max-w-64 truncate" title={selected.mountPoint}>
                {selected.mountPoint}
              </span>
            </span>
          </>
        ) : (
          <span className="text-muted-foreground">{formatCount(rows.length, "Volume")}</span>
        )}
      </div>
    )
  }

  const directoryCount = rows.filter((row) => row.kind === "directory").length
  const fileCount = rows.filter((row) => row.kind === "file").length
  const totalSelectedSize = selectedRows.reduce((total, row) => total + row.size, 0)
  const canDeleteSelection = selectedRows.every((row) => row.canDeleteNow !== false)
  const canPreviewSelection = selectedRows.length === 1 && selected?.kind === "file"
  const handleOpenLocation = () => {
    void openItemLocation(selectedRows.map((row) => row.path)).catch(() => undefined)
  }
  const handlePreview = () => {
    if (!selected || selected.kind !== "file") {
      return
    }

    void previewItem(selected.path).catch(() => undefined)
  }

  return (
    <div className="flex min-h-9 items-center justify-between gap-3 border-t bg-muted/50 px-3 text-xs">
      <span className="min-w-0 truncate">
        {selectedRows.length === 0
          ? `${formatCount(directoryCount, "Directory")}, ${formatCount(fileCount, "File")}`
          : selectedRows.length === 1 && selected
            ? `${selected.label} (${formatBytes(selected.size)})`
            : `${formatCount(selectedRows.length, "item")} selected (${formatBytes(totalSelectedSize)})`}
      </span>
      {selectedRows.length > 0 ? (
        <div className="flex shrink-0 items-center gap-1">
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label="Delete selected items"
            disabled={!canDeleteSelection}
            onClick={noopAction}
          >
            <Trash2Icon data-icon="inline-start" />
          </Button>
          <Button type="button" variant="ghost" size="icon-sm" aria-label="Open selected item location" onClick={handleOpenLocation}>
            <FolderOpenIcon data-icon="inline-start" />
          </Button>
          {selectedRows.length === 1 ? (
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label="Preview selected item"
              disabled={!canPreviewSelection}
              onClick={handlePreview}
            >
              <EyeIcon data-icon="inline-start" />
            </Button>
          ) : null}
        </div>
      ) : null}
    </div>
  )
}

function volumeToStatusItem(volume: DriveDto): StatusItem {
  return {
    id: volume.id,
    label: volume.label,
    kind: "volume",
    size: volume.usedSpace,
    totalSpace: volume.totalSpace,
    availableSpace: volume.availableSpace,
    fileSystem: volume.fileSystem,
    mountPoint: volume.mountPoint,
    path: volume.mountPoint,
  }
}

function nodeToStatusItem(node: PathNodeDto): StatusItem {
  return {
    id: node.path,
    label: node.name,
    kind: node.kind,
    size: node.logicalSize,
    mountPoint: node.path,
    path: node.path,
    canDeleteNow: node.deleteSafety?.canDeleteNow,
  }
}

function formatCount(count: number, noun: string) {
  if (count === 1) {
    return `${count} ${noun}`
  }

  const plural = noun.endsWith("y") ? `${noun.slice(0, -1)}ies` : `${noun}s`
  return `${count} ${plural}`
}

function noopAction() {}

function tableRowClassName(className?: string) {
  return cn("grid grid-cols-[minmax(0,1fr)_max-content]", className)
}

function selectableRowClassName(className: string, selected: boolean) {
  return tableRowClassName(cn(className, selected && "bg-muted/50 hover:bg-muted/50"))
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
        "cursor-pointer select-none whitespace-nowrap bg-background transition-colors hover:bg-muted/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
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
