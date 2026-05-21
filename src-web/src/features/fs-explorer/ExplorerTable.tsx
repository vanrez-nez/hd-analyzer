import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  ArrowDownIcon,
  ArrowUpIcon,
  EyeIcon,
  FileIcon,
  Folder,
  FolderOpenIcon,
  HardDrive,
  LockIcon,
  SquareTerminalIcon,
  Trash2Icon,
} from "lucide-react"
import AutoSizer from "react-virtualized/dist/es/AutoSizer"
import List, { type ListRowProps } from "react-virtualized/dist/es/List"

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Badge } from "@/components/ui/badge"
import { Button, buttonVariants } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { Table, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { openFolder, openItemLocation, openTerminal, previewItem } from "@/api"
import { isToggleSelectionInput, replaceSelection, toggleSelection } from "@/lib/selection"
import { cn } from "@/lib/utils"
import { ExplorerLoadingOverlay, type ExplorerLoadingOverlayProps } from "./ExplorerLoadingOverlay"
import type {
  DeleteSafetyClassification,
  DirectoryListingDto,
  DirectoryProgressUpdateDto,
  DriveDto,
  PathNodeDto,
} from "./types"
import type { CSSProperties, MouseEvent } from "react"

type SortColumn = "name" | "size"
type SortDirectionState = "asc" | "desc"
type SortState = {
  column: SortColumn
  direction: SortDirectionState
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
  isReadOnly?: boolean
}
type ExplorerBusyOverlay = ExplorerLoadingOverlayProps
type ExplorerRow = DriveDto | PathNodeDto
type RowCounts = {
  directories: number
  files: number
  volumes: number
}
type RowsRenderedRange = {
  startIndex: number
  stopIndex: number
}
type ExplorerTableStyle = CSSProperties & {
  "--explorer-size-column-width": string
}

const ROW_HEIGHT = 32
const OVERSCAN_ROW_COUNT = 24
const ROW_ICON_CLASS = "size-3.5 shrink-0"
const EXPLORER_ROW_SELECTOR = "[data-explorer-row]"
const SIZE_COLUMN_MIN_WIDTH = 72
const SIZE_COLUMN_MAX_WIDTH = 160
const SIZE_COLUMN_HORIZONTAL_PADDING = 24
const SIZE_COLUMN_SORT_AFFORDANCE_WIDTH = 18
const SIZE_COLUMN_SPINNER_WIDTH = 16
const SIZE_COLUMN_TEXT_FONT = '400 12px Inter, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
const SIZE_COLUMN_HEADER_FONT = '500 12px Inter, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
const SIZE_COLUMN_FALLBACK_CHAR_WIDTH = 7

type ExplorerTableProps = {
  busyOverlay?: ExplorerBusyOverlay
  className?: string
  liveUpdates?: Record<string, DirectoryProgressUpdateDto>
  volumes: DriveDto[]
  listing?: DirectoryListingDto
  loadingPath?: string
  selectedItemIds: string[]
  selectionAnchorId: string | null
  onDeleteItems: (paths: string[], moveToTrash: boolean) => Promise<void>
  onOpenVolume: (volume: DriveDto) => void
  onOpenNode: (node: PathNodeDto) => void
  onSelectionAnchorChange: (itemId: string | null) => void
  onSelectionChange: (itemIds: string[]) => void
}

export function ExplorerTable({
  busyOverlay,
  className,
  liveUpdates,
  volumes,
  listing,
  loadingPath,
  selectedItemIds,
  selectionAnchorId,
  onDeleteItems,
  onOpenVolume,
  onOpenNode,
  onSelectionAnchorChange,
  onSelectionChange,
}: ExplorerTableProps) {
  const listRef = useRef<List | null>(null)
  const [sort, setSort] = useState<SortState>({ column: "size", direction: "desc" })
  const [scrollToIndex, setScrollToIndex] = useState<number>()
  const rows = useMemo<ExplorerRow[]>(() => {
    return listing ? sortNodes(listing.children, sort) : sortVolumes(volumes, sort)
  }, [listing, sort, volumes])
  const rowCounts = useMemo(() => countRows(rows), [rows])
  const selectedItemSet = useMemo(() => new Set(selectedItemIds), [selectedItemIds])
  const selectedStatusItems = useMemo(
    () => selectedStatusItemsForRows(rows, selectedItemIds, liveUpdates),
    [liveUpdates, rows, selectedItemIds],
  )
  const sizeColumnWidth = useMemo(
    () => calculateSizeColumnWidth(rows, loadingPath, liveUpdates),
    [liveUpdates, loadingPath, rows],
  )
  const tableStyle = useMemo<ExplorerTableStyle>(
    () => ({
      "--explorer-size-column-width": `${sizeColumnWidth}px`,
    }),
    [sizeColumnWidth],
  )

  useEffect(() => {
    listRef.current?.forceUpdateGrid()
  }, [liveUpdates, loadingPath, selectedItemIds])

  useEffect(() => {
    if (selectedItemIds.length === 0) {
      setScrollToIndex(undefined)
      return
    }

    const nextIndex = findFirstSelectedRowIndex(rows, selectedItemSet)
    setScrollToIndex(nextIndex === -1 ? undefined : nextIndex)
  }, [rows, selectedItemIds, selectedItemSet])

  const selectRow = useCallback(
    (row: ExplorerRow, index: number, event: MouseEvent<HTMLElement>) => {
      const itemId = rowId(row)
      if (!isPathNode(row)) {
        onSelectionChange(replaceSelection([itemId]))
        onSelectionAnchorChange(itemId)
        return
      }

      if (event.shiftKey) {
        onSelectionChange(rangeSelectionFromRows(rows, selectionAnchorId, index))
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
    },
    [onSelectionAnchorChange, onSelectionChange, rows, selectedItemIds, selectionAnchorId],
  )

  const openRow = useCallback(
    (row: ExplorerRow) => {
      if (isPathNode(row)) {
        if (row.kind === "directory") {
          onOpenNode(row)
        }
        return
      }

      onOpenVolume(row)
    },
    [onOpenNode, onOpenVolume],
  )

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

  const handleRowsRendered = useCallback(
    ({ startIndex, stopIndex }: RowsRenderedRange) => {
      if (scrollToIndex === undefined) {
        return
      }

      if (scrollToIndex >= startIndex && scrollToIndex <= stopIndex) {
        setScrollToIndex(undefined)
      }
    },
    [scrollToIndex],
  )

  const renderRow = useCallback(
    ({ index, key, style }: ListRowProps) => {
      const row = rows[index]
      if (!row) {
        return null
      }

      const selected = selectedItemSet.has(rowId(row))

      return (
        <div
          aria-selected={selected}
          className={selectableRowClassName(
            !isPathNode(row) || row.kind === "directory" ? "cursor-pointer select-none" : "select-none",
            selected,
          )}
          data-explorer-row
          key={key}
          role="row"
          style={style}
          title={rowTitle(row)}
          onClick={(event) => selectRow(row, index, event)}
          onDoubleClick={() => openRow(row)}
        >
          <div className="flex h-full min-w-0 items-center overflow-hidden whitespace-nowrap px-3">
            <NameCell row={row} />
          </div>
          <div className="flex h-full items-center justify-end whitespace-nowrap px-3 text-right tabular-nums">
            {isPathNode(row) ? renderNodeSize(row, loadingPath, liveUpdates?.[row.path]) : formatBytes(row.usedSpace)}
          </div>
        </div>
      )
    },
    [liveUpdates, loadingPath, openRow, rows, selectRow, selectedItemSet],
  )

  return (
    <div
      aria-busy={Boolean(busyOverlay)}
      className={cn("relative flex min-h-0 flex-1 flex-col overflow-hidden rounded-md border text-xs", className)}
      style={tableStyle}
    >
      <div className={cn("flex min-h-0 flex-1 flex-col overflow-hidden", busyOverlay && "opacity-[0.15]")}>
        <Table className="block w-full text-xs">
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
        <div className="min-h-0 flex-1 overflow-hidden overscroll-none" onClick={clearSelectionOnEmptyListClick}>
          <AutoSizer>
            {({ height, width }) => {
              if (height <= 0 || width <= 0) {
                return null
              }

              return (
                <List
                  aria-label="File explorer"
                  className="outline-none"
                  height={height}
                  noRowsRenderer={renderNoRows}
                  overscanRowCount={OVERSCAN_ROW_COUNT}
                  ref={listRef}
                  rowCount={rows.length}
                  rowHeight={ROW_HEIGHT}
                  rowRenderer={renderRow}
                  scrollToAlignment="auto"
                  scrollToIndex={scrollToIndex ?? -1}
                  style={{ overflowX: "hidden", overscrollBehavior: "none" }}
                  tabIndex={0}
                  width={width}
                  onRowsRendered={handleRowsRendered}
                />
              )
            }}
          </AutoSizer>
        </div>
        <ExplorerStatusBar
          listing={listing}
          rowCounts={rowCounts}
          selectedRows={selectedStatusItems}
          onDeleteItems={onDeleteItems}
        />
      </div>
      {busyOverlay ? <ExplorerLoadingOverlay {...busyOverlay} /> : null}
    </div>
  )
}

function NameCell({ row }: { row: ExplorerRow }) {
  if (!isPathNode(row)) {
    return (
      <span className="flex min-w-0 items-center gap-2 overflow-hidden">
        <HardDrive data-icon="inline-start" className={ROW_ICON_CLASS} />
        <span className="truncate" title={volumeDisplayName(row)}>
          {volumeDisplayName(row)}
        </span>
      </span>
    )
  }

  return (
    <span className="flex min-w-0 items-center gap-2 overflow-hidden">
      {row.kind === "directory" ? (
        <Folder data-icon="inline-start" className={cn(ROW_ICON_CLASS, safetyIconClass(row.deleteSafety?.classification))} />
      ) : (
        <FileIcon data-icon="inline-start" className={cn(ROW_ICON_CLASS, "opacity-50")} />
      )}
      <span
        className={cn("truncate", row.kind === "directory" && safetyTextClass(row.deleteSafety?.classification))}
        title={row.deleteSafety?.reason ? `${row.name} - ${row.deleteSafety.reason}` : row.name}
      >
        {row.name}
      </span>
    </span>
  )
}

function ExplorerStatusBar({
  listing,
  onDeleteItems,
  rowCounts,
  selectedRows,
}: {
  listing?: DirectoryListingDto
  onDeleteItems: (paths: string[], moveToTrash: boolean) => Promise<void>
  rowCounts: RowCounts
  selectedRows: StatusItem[]
}) {
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)
  const [moveToTrash, setMoveToTrash] = useState(true)
  const selected = selectedRows[0]
  const isRootVolumes = !listing

  if (isRootVolumes) {
    return (
      <div className="flex min-h-9 select-none items-center justify-between gap-3 border-t bg-muted/50 px-3 text-xs">
        {selected ? (
          <>
            <span className="min-w-0 truncate font-medium" title={selected.label}>
              {selected.label}
            </span>
            <span className="flex min-w-0 items-center justify-end gap-1.5">
              <Badge className="shrink-0 whitespace-nowrap">Total {formatBytes(selected.totalSpace ?? 0)}</Badge>
              <Badge className="shrink-0 whitespace-nowrap">Free {formatBytes(selected.availableSpace ?? 0)}</Badge>
              <Badge className="max-w-28 shrink-0 truncate" title={selected.fileSystem || "Unknown FS"}>
                {selected.fileSystem || "Unknown FS"}
              </Badge>
              <Badge className="max-w-48 shrink truncate" title={selected.mountPoint}>
                {selected.mountPoint}
              </Badge>
              {selected.isReadOnly ? (
                <Badge className="shrink-0 whitespace-nowrap">
                  <LockIcon className="mr-1 size-3" aria-hidden="true" />
                  Read-only
                </Badge>
              ) : null}
            </span>
          </>
        ) : (
          <span className="text-muted-foreground">{formatCount(rowCounts.volumes, "Volume")}</span>
        )}
      </div>
    )
  }

  const totalSelectedSize = selectedRows.reduce((total, row) => total + row.size, 0)
  const canDeleteSelection = selectedRows.every((row) => row.canDeleteNow !== false)
  const canPreviewSelection = selectedRows.length === 1 && selected?.kind === "file"
  const deleteDialogTitle =
    selectedRows.length === 1 && selected
      ? `Are you sure to delete: ${selected.label}`
      : `Are you sure to delete: ${selectedRows.length} selected files`
  const openLocationLabel = selectedRows.length > 0 ? "Open selected item location" : "Open current folder location"
  const handleDeleteClick = () => {
    setMoveToTrash(true)
    setDeleteDialogOpen(true)
  }
  const handleConfirmDelete = async () => {
    if (selectedRows.length === 0 || !canDeleteSelection) {
      return
    }

    setIsDeleting(true)
    try {
      await onDeleteItems(selectedRows.map((row) => row.path), moveToTrash)
      setDeleteDialogOpen(false)
    } catch {
      // FsExplorer owns surfacing the command error in the explorer error area.
    } finally {
      setIsDeleting(false)
    }
  }
  const handleOpenLocation = () => {
    if (selectedRows.length === 0) {
      void openFolder(listing.path).catch(() => undefined)
      return
    }

    void openItemLocation(selectedRows.map((row) => row.path)).catch(() => undefined)
  }
  const handleOpenTerminal = () => {
    void openTerminal(listing.path).catch(() => undefined)
  }
  const handlePreview = () => {
    if (!selected || selected.kind !== "file") {
      return
    }

    void previewItem(selected.path).catch(() => undefined)
  }

  return (
    <div className="flex min-h-9 select-none items-center justify-between gap-3 border-t bg-muted/50 px-3 text-xs">
      <span className="min-w-0 truncate">
        {selectedRows.length === 0
          ? `${formatCount(rowCounts.directories, "Directory")}, ${formatCount(rowCounts.files, "File")}`
          : selectedRows.length === 1 && selected
            ? `${selected.label} (${formatBytes(selected.size)})`
            : `${formatCount(selectedRows.length, "item")} selected (${formatBytes(totalSelectedSize)})`}
      </span>
      <div className="flex shrink-0 items-center gap-1">
        {selectedRows.length > 0 ? (
          <AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label="Delete selected items"
              disabled={!canDeleteSelection || isDeleting}
              onClick={handleDeleteClick}
            >
              <Trash2Icon data-icon="inline-start" />
            </Button>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>{deleteDialogTitle}</AlertDialogTitle>
                <AlertDialogDescription className="sr-only">
                  Confirm deletion for the selected filesystem item(s).
                </AlertDialogDescription>
              </AlertDialogHeader>
              <div className="flex items-center justify-between gap-4">
                <label className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Switch checked={moveToTrash} disabled={isDeleting} onCheckedChange={setMoveToTrash} />
                  <span>Move to trash</span>
                </label>
                <div className="flex items-center gap-2">
                  <AlertDialogCancel disabled={isDeleting}>Cancel</AlertDialogCancel>
                  <AlertDialogAction
                    className={buttonVariants({ variant: "destructive" })}
                    disabled={isDeleting}
                    onClick={(event) => {
                      event.preventDefault()
                      void handleConfirmDelete()
                    }}
                  >
                    Delete
                  </AlertDialogAction>
                </div>
              </div>
            </AlertDialogContent>
          </AlertDialog>
        ) : null}
        <Button type="button" variant="ghost" size="icon-sm" aria-label={openLocationLabel} onClick={handleOpenLocation}>
          <FolderOpenIcon data-icon="inline-start" />
        </Button>
        <Button type="button" variant="ghost" size="icon-sm" aria-label="Open terminal in current folder" onClick={handleOpenTerminal}>
          <SquareTerminalIcon data-icon="inline-start" />
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
    </div>
  )
}

function countRows(rows: ExplorerRow[]): RowCounts {
  let directories = 0
  let files = 0
  let volumes = 0

  for (const row of rows) {
    if (!isPathNode(row)) {
      volumes += 1
    } else if (row.kind === "directory") {
      directories += 1
    } else if (row.kind === "file") {
      files += 1
    }
  }

  return { directories, files, volumes }
}

function selectedStatusItemsForRows(
  rows: ExplorerRow[],
  selectedItemIds: string[],
  liveUpdates?: Record<string, DirectoryProgressUpdateDto>,
) {
  if (selectedItemIds.length === 0) {
    return []
  }

  if (selectedItemIds.length > 32) {
    const selected = new Set(selectedItemIds)
    return rows.filter((row) => selected.has(rowId(row))).map((row) => rowToStatusItem(row, liveUpdates))
  }

  return selectedItemIds.flatMap((itemId) => {
    const row = rows[findRowIndexById(rows, itemId)]
    return row ? [rowToStatusItem(row, liveUpdates)] : []
  })
}

function rowToStatusItem(row: ExplorerRow, liveUpdates?: Record<string, DirectoryProgressUpdateDto>): StatusItem {
  if (isPathNode(row)) {
    return nodeToStatusItem(row, liveUpdates?.[row.path])
  }

  return volumeToStatusItem(row)
}

function volumeToStatusItem(volume: DriveDto): StatusItem {
  return {
    id: volume.id,
    label: volumeDisplayName(volume),
    kind: "volume",
    size: volume.usedSpace,
    totalSpace: volume.totalSpace,
    availableSpace: volume.availableSpace,
    fileSystem: volume.fileSystem,
    isReadOnly: volume.isReadOnly,
    mountPoint: volume.mountPoint,
    path: volume.mountPoint,
  }
}

function nodeToStatusItem(node: PathNodeDto, update?: DirectoryProgressUpdateDto): StatusItem {
  return {
    id: node.path,
    label: node.name,
    kind: node.kind,
    size: update?.logicalSize ?? node.logicalSize,
    mountPoint: node.path,
    path: node.path,
    canDeleteNow: node.deleteSafety?.canDeleteNow,
  }
}

function rangeSelectionFromRows(rows: ExplorerRow[], anchorItemId: string | null, targetIndex: number) {
  const target = rows[targetIndex]
  const anchorIndex = anchorItemId ? findRowIndexById(rows, anchorItemId) : -1

  if (!target || anchorIndex === -1) {
    return target ? [rowId(target)] : []
  }

  const start = Math.min(anchorIndex, targetIndex)
  const end = Math.max(anchorIndex, targetIndex)
  return rows.slice(start, end + 1).map(rowId)
}

function findFirstSelectedRowIndex(rows: ExplorerRow[], selectedItemSet: Set<string>) {
  if (selectedItemSet.size === 0) {
    return -1
  }

  for (let index = 0; index < rows.length; index += 1) {
    if (selectedItemSet.has(rowId(rows[index]))) {
      return index
    }
  }

  return -1
}

function findRowIndexById(rows: ExplorerRow[], itemId: string) {
  for (let index = 0; index < rows.length; index += 1) {
    if (rowId(rows[index]) === itemId) {
      return index
    }
  }

  return -1
}

function rowId(row: ExplorerRow) {
  return isPathNode(row) ? row.path : row.id
}

function rowTitle(row: ExplorerRow) {
  return isPathNode(row) ? row.deleteSafety?.reason : undefined
}

function isPathNode(row: ExplorerRow): row is PathNodeDto {
  return "childrenKnown" in row
}

function formatCount(count: number, noun: string) {
  if (count === 1) {
    return `${count} ${noun}`
  }

  const plural = noun.endsWith("y") ? `${noun.slice(0, -1)}ies` : `${noun}s`
  return `${count} ${plural}`
}

function calculateSizeColumnWidth(
  rows: ExplorerRow[],
  loadingPath?: string,
  liveUpdates?: Record<string, DirectoryProgressUpdateDto>,
) {
  let maxContentWidth =
    measureSizeColumnText("Size", SIZE_COLUMN_HEADER_FONT) + SIZE_COLUMN_SORT_AFFORDANCE_WIDTH

  for (const row of rows) {
    maxContentWidth = Math.max(maxContentWidth, measureSizeLabelWidth(row, loadingPath, liveUpdates))
  }

  return clamp(
    Math.ceil(maxContentWidth + SIZE_COLUMN_HORIZONTAL_PADDING),
    SIZE_COLUMN_MIN_WIDTH,
    SIZE_COLUMN_MAX_WIDTH,
  )
}

function measureSizeLabelWidth(
  row: ExplorerRow,
  loadingPath?: string,
  liveUpdates?: Record<string, DirectoryProgressUpdateDto>,
) {
  if (!isPathNode(row)) {
    return measureSizeColumnText(formatBytes(row.usedSpace), SIZE_COLUMN_TEXT_FONT)
  }

  const update = liveUpdates?.[row.path]
  const state = update?.state ?? row.state
  const logicalSize = update?.logicalSize ?? row.logicalSize

  if ((isWorking(state) || loadingPath === row.path) && logicalSize <= 0) {
    return SIZE_COLUMN_SPINNER_WIDTH
  }

  if (state !== "complete" && logicalSize <= 0) {
    return 0
  }

  return measureSizeColumnText(formatBytes(logicalSize), SIZE_COLUMN_TEXT_FONT)
}

let sizeColumnMeasureContext: CanvasRenderingContext2D | null | undefined

function measureSizeColumnText(text: string, font: string) {
  const context = getSizeColumnMeasureContext()

  if (!context) {
    return text.length * SIZE_COLUMN_FALLBACK_CHAR_WIDTH
  }

  context.font = font
  return context.measureText(text).width
}

function getSizeColumnMeasureContext() {
  if (sizeColumnMeasureContext !== undefined) {
    return sizeColumnMeasureContext
  }

  if (typeof document === "undefined") {
    sizeColumnMeasureContext = null
    return sizeColumnMeasureContext
  }

  sizeColumnMeasureContext = document.createElement("canvas").getContext("2d")
  return sizeColumnMeasureContext
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}

function tableRowClassName(className?: string) {
  return cn("grid select-none grid-cols-[minmax(0,1fr)_var(--explorer-size-column-width)]", className)
}

function selectableRowClassName(className: string, selected: boolean) {
  return tableRowClassName(cn(className, selected && "bg-muted/50 hover:bg-muted/50"))
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
      return volumeDisplayName(volume)
  }
}

function volumeDisplayName(volume: DriveDto) {
  const name = volumeNameOnly(volume.label, volume.mountPoint)
  const location = volume.isRemovable ? "External" : "Internal"
  const kind = normalizeStorageKind(volume.storageKind)
  return kind ? `${name} (${location} ${kind})` : `${name} (${location})`
}

function normalizeStorageKind(kind: string) {
  const normalized = kind.trim()
  return normalized && normalized.toLowerCase() !== "unknown" ? normalized : undefined
}

function volumeNameOnly(label: string, rootPath: string) {
  const trimmed = label.trim()
  const normalizedRoot = normalizePath(rootPath)
  const suffixMatch = trimmed.match(/^(.*?)\s+\((.*)\)$/)
  if (suffixMatch?.[2] && normalizePath(suffixMatch[2]) === normalizedRoot) {
    return suffixMatch[1].trim() || lastPathPart(normalizedRoot)
  }

  if (normalizePath(trimmed) === normalizedRoot || trimmed.startsWith("/")) {
    return lastPathPart(normalizedRoot)
  }

  return trimmed || lastPathPart(normalizedRoot)
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

function renderNodeSize(node: PathNodeDto, loadingPath?: string, update?: DirectoryProgressUpdateDto) {
  const state = update?.state ?? node.state
  const logicalSize = update?.logicalSize ?? node.logicalSize

  if (isWorking(state) || loadingPath === node.path) {
    if (logicalSize > 0) {
      return formatBytes(logicalSize)
    }

    return (
      <span className="inline-flex w-full justify-end">
        <Spinner />
      </span>
    )
  }

  if (state !== "complete") return ""
  return formatBytes(logicalSize)
}

function renderNoRows() {
  return <div className="flex h-full items-center px-3 text-xs text-muted-foreground">No items</div>
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
