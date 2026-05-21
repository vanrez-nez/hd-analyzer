import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { PanelRightOpenIcon, RefreshCwIcon, ShieldCheck } from "lucide-react"

import { deleteItems, fsInvalidatePath, fsListVolumes, fsOpenPathWithProgress, fsStartScan } from "@/api"
import { ButtonGroup } from "@/components/ui/button-group"
import { Button } from "@/components/ui/button"
import { ExplorerTable } from "./ExplorerTable"
import { PathButtonGroup } from "./PathButtonGroup"
import { VolumeDetails } from "./VolumeDetails"
import { createExplorerCache, getCachedListing, markPathStale, putCachedListing } from "./cache"
import type { VisualizerCellInput, VisualizerLevelSnapshot } from "@/features/visualizer/types"
import type {
  DirectoryListingDto,
  DirectoryProgressUpdateDto,
  DriveDto,
  ExplorerCache,
  FsProgressEvent,
  PathNodeDto,
  ProgressSnapshotDto,
} from "./types"
import { defaultScanConfig } from "./types"

type LiveDirectoryUpdates = Record<string, Record<string, DirectoryProgressUpdateDto>>
type PendingNavigation = {
  entriesProcessed: number
  path: string
  phase: "opening" | "scanning"
}
type ScanProgressState = {
  path: string
  snapshot: ProgressSnapshotDto
}

type FsExplorerProps = {
  explorerSelectionAnchorId: string | null
  permissionChecking: boolean
  selectedItemIds: string[]
  tableClassName?: string
  visualizerOpen: boolean
  onExplorerSelectionAnchorChange: (itemId: string | null) => void
  onRequestPermissions: () => void
  onSelectionChange: (itemIds: string[]) => void
  onVisualizerToggle: () => void
  onVisualizerSnapshotChange?: (snapshot: VisualizerLevelSnapshot) => void
}

export function FsExplorer({
  explorerSelectionAnchorId,
  permissionChecking,
  selectedItemIds,
  tableClassName,
  visualizerOpen,
  onExplorerSelectionAnchorChange,
  onRequestPermissions,
  onSelectionChange,
  onVisualizerToggle,
  onVisualizerSnapshotChange,
}: FsExplorerProps) {
  const [volumes, setVolumes] = useState<DriveDto[]>([])
  const [selectedVolume, setSelectedVolume] = useState<DriveDto>()
  const [currentPath, setCurrentPath] = useState<string>()
  const [cache, setCache] = useState<ExplorerCache>(() => createExplorerCache())
  const [loadingPath, setLoadingPath] = useState<string>()
  const [pendingNavigation, setPendingNavigation] = useState<PendingNavigation>()
  const [scanProgress, setScanProgress] = useState<ScanProgressState>()
  const [liveUpdates, setLiveUpdates] = useState<LiveDirectoryUpdates>({})
  const [error, setError] = useState<string>()
  const activeScansRef = useRef<Set<string>>(new Set())
  const scanJobPathRef = useRef<Map<string, string>>(new Map())
  const navigationRequestRef = useRef(0)

  const listing = useMemo(
    () => (currentPath ? getCachedListing(cache, currentPath) : undefined),
    [cache, currentPath],
  )
  const listingLiveUpdates = listing ? liveUpdates[listing.path] : undefined
  const visualizerLiveUpdates = visualizerOpen ? listingLiveUpdates : undefined
  const canReloadCurrentPath = Boolean(selectedVolume && currentPath)
  const isNavigationPending = Boolean(pendingNavigation)
  const isReloadingCurrentPath = Boolean(currentPath && loadingPath === currentPath)
  const isVolumesLevel = !currentPath
  const busyOverlay = useMemo(
    () =>
      pendingNavigation
        ? {
            ...pendingNavigation,
            infinite: true,
            operationKey: `${pendingNavigation.phase}:${pendingNavigation.path}`,
            ...progressSnapshotToOverlayProps(scanProgress, pendingNavigation.path),
          }
        : undefined,
    [pendingNavigation, scanProgress],
  )

  const refreshVolumes = useCallback(async () => {
    const nextVolumes = await fsListVolumes()
    setVolumes(nextVolumes)
    setSelectedVolume((current) => {
      if (!current) {
        return current
      }

      return nextVolumes.find((volume) => isSameVolume(volume, current)) ?? current
    })
  }, [])

  useEffect(() => {
    refreshVolumes().catch((error: unknown) => setError(error instanceof Error ? error.message : String(error)))
  }, [currentPath, refreshVolumes])

  useEffect(() => {
    if (!onVisualizerSnapshotChange) {
      return
    }

    if (!currentPath) {
      const items = volumes.map(volumeToVisualizerItem)
      onVisualizerSnapshotChange({
        path: null,
        parentPath: null,
        items,
        generation: createVisualizerGeneration(null, items),
      })
      return
    }

    const items = listing
      ? listing.children.filter((node) => node.visible).map((node) => nodeToVisualizerItem(node, visualizerLiveUpdates?.[node.path]))
      : []
    onVisualizerSnapshotChange({
      path: currentPath,
      parentPath: selectedVolume ? getVisualizerParentPath(currentPath, selectedVolume.mountPoint) : null,
      items,
      generation: createVisualizerGeneration(currentPath, items),
    })
  }, [currentPath, listing, onVisualizerSnapshotChange, selectedVolume, visualizerLiveUpdates, volumes])

  const mergeListing = useCallback((listing: DirectoryListingDto) => {
    setCache((cache) => putCachedListing(cache, listing))
    setLiveUpdates((updates) => {
      if (!updates[listing.path]) {
        return updates
      }

      const { [listing.path]: _finished, ...remaining } = updates
      return remaining
    })
  }, [])

  const handleProgress = useCallback(
    (event: FsProgressEvent) => {
      if (event.event === "jobQueued" || event.event === "jobStarted") {
        scanJobPathRef.current.set(progressEventKey(event.data.jobId, event.data.requestId), event.data.path)
      }
      if (event.event === "directoryReady") {
        scanJobPathRef.current.set(progressEventKey(event.data.jobId, event.data.requestId), event.data.path)
        mergeListing(event.data.listing)
      }
      if (event.event === "progressSnapshot") {
        const progressPath = scanJobPathRef.current.get(progressEventKey(event.data.jobId, event.data.requestId))
        if (progressPath) {
          setScanProgress({ path: progressPath, snapshot: event.data.snapshot })
        }
        setPendingNavigation((pending) =>
          pending && progressPath === pending.path
            ? { ...pending, entriesProcessed: event.data.snapshot.completedUnits, phase: "scanning" }
            : pending,
        )
      }
      if (event.event === "directoryProgress") {
        scanJobPathRef.current.set(progressEventKey(event.data.jobId, event.data.requestId), event.data.path)
        setScanProgress({ path: event.data.path, snapshot: event.data.snapshot })
        setPendingNavigation((pending) =>
          pending && event.data.path === pending.path
            ? { ...pending, entriesProcessed: event.data.snapshot.completedUnits, phase: "scanning" }
            : pending,
        )
        setLiveUpdates((updates) => ({
          ...updates,
          [event.data.path]: {
            ...(updates[event.data.path] ?? {}),
            ...Object.fromEntries(event.data.updates.map((update) => [update.path, update])),
          },
        }))
      }
      if (event.event === "jobFinished" || event.event === "jobFailed") {
        scanJobPathRef.current.delete(progressEventKey(event.data.jobId, event.data.requestId))
        activeScansRef.current.delete(scanKey(event.data.path))
        setLoadingPath((loadingPath) => (loadingPath === event.data.path ? undefined : loadingPath))
        setScanProgress((progress) => (progress?.path === event.data.path ? undefined : progress))
        setPendingNavigation((pending) => (pending?.path === event.data.path ? undefined : pending))
        setLiveUpdates((updates) => {
          if (!updates[event.data.path]) {
            return updates
          }

          const { [event.data.path]: _finished, ...remaining } = updates
          return remaining
        })
      }
    },
    [mergeListing],
  )

  const startScan = useCallback(
    async (path: string, volumeRoot: string, replaceExisting = false) => {
      const key = scanKey(path)
      if (!replaceExisting && activeScansRef.current.has(key)) {
        return false
      }

      activeScansRef.current.add(key)
      setScanProgress({
        path,
        snapshot: {
          jobId: "",
          requestId: "",
          state: "queued",
          estimatedTotalBytes: null,
          scheduledUnits: 0,
          discoveredUnits: 0,
          completedUnits: 0,
          activeUnits: 0,
          skippedUnits: 0,
          failedUnits: 0,
          canceledUnits: 0,
          bytesMeasured: 0,
          activePaths: [path],
        },
      })
      try {
        await fsStartScan(path, volumeRoot, defaultScanConfig, handleProgress, replaceExisting)
        return true
      } catch (error) {
        activeScansRef.current.delete(key)
        throw error
      }
    },
    [handleProgress],
  )

  const openPath = useCallback(
    async (path: string, volume = selectedVolume, forceRefresh = false) => {
      if (!volume) {
        return
      }
      const volumeRoot = volume.mountPoint
      const cached = getCachedListing(cache, path)
      if (cached && !forceRefresh) {
        navigationRequestRef.current += 1
        setCurrentPath(cached.path)
        setPendingNavigation(undefined)
        setLoadingPath(undefined)
        setError(undefined)
        return
      }

      const requestId = navigationRequestRef.current + 1
      navigationRequestRef.current = requestId
      setError(undefined)
      setLoadingPath(path)
      setPendingNavigation({ entriesProcessed: 0, path, phase: "opening" })
      try {
        const listing = await fsOpenPathWithProgress(path, volumeRoot, defaultScanConfig, (progress) => {
          if (navigationRequestRef.current !== requestId) {
            return
          }

          setPendingNavigation((pending) =>
            pending?.path === path
              ? { ...pending, entriesProcessed: progress.entriesProcessed, phase: "opening" }
              : pending,
          )
        })
        if (navigationRequestRef.current !== requestId) {
          return
        }

        mergeListing(listing)
        setCurrentPath(listing.path)
        if (listingNeedsScan(listing)) {
          setLoadingPath(listing.path)
          setPendingNavigation({ entriesProcessed: 0, path: listing.path, phase: "scanning" })
          const started = await startScan(listing.path, volumeRoot)
          if (!started) {
            setPendingNavigation(undefined)
            setLoadingPath(undefined)
          }
        } else {
          setPendingNavigation(undefined)
          setLoadingPath(undefined)
        }
      } catch (error) {
        if (navigationRequestRef.current !== requestId) {
          return
        }

        setError(error instanceof Error ? error.message : String(error))
        setPendingNavigation(undefined)
        setLoadingPath(undefined)
      }
    },
    [cache, mergeListing, selectedVolume, startScan],
  )

  const deleteExplorerItems = useCallback(
    async (paths: string[], moveToTrash: boolean) => {
      if (!selectedVolume || !currentPath || paths.length === 0) {
        return
      }

      setError(undefined)
      try {
        await deleteItems(paths, selectedVolume.mountPoint, moveToTrash)
        await fsInvalidatePath(currentPath, selectedVolume.mountPoint)
        setCache((cache) => markPathStale(cache, currentPath, true))
        onSelectionChange([])
        onExplorerSelectionAnchorChange(null)
        await openPath(currentPath, selectedVolume, true)
        await refreshVolumes().catch((error: unknown) => {
          setError(error instanceof Error ? error.message : String(error))
        })
      } catch (error) {
        setError(error instanceof Error ? error.message : String(error))
        throw error
      }
    },
    [currentPath, onExplorerSelectionAnchorChange, onSelectionChange, openPath, refreshVolumes, selectedVolume],
  )

  const openVolume = (volume: DriveDto) => {
    setSelectedVolume(volume)
    void openPath(volume.mountPoint, volume)
  }

  const openNode = (node: PathNodeDto) => {
    if (node.kind === "directory") {
      void openPath(node.path)
    }
  }

  const reloadCurrentPath = useCallback(async () => {
    if (!selectedVolume || !currentPath) {
      return
    }

    setError(undefined)
    setLoadingPath(currentPath)
    setPendingNavigation({ entriesProcessed: 0, path: currentPath, phase: "scanning" })
    try {
      const started = await startScan(currentPath, selectedVolume.mountPoint, true)
      if (!started) {
        setPendingNavigation(undefined)
        setLoadingPath(undefined)
      }
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error))
      setPendingNavigation(undefined)
      setLoadingPath(undefined)
    }
  }, [currentPath, selectedVolume, startScan])

  const returnToVolumes = () => {
    navigationRequestRef.current += 1
    setSelectedVolume(undefined)
    setCurrentPath(undefined)
    setLoadingPath(undefined)
    setPendingNavigation(undefined)
    setError(undefined)
  }

  return (
    <section className="flex h-full min-h-0 flex-1 flex-col gap-3 overflow-hidden">
      <div className="flex min-h-8 items-center gap-2 overflow-hidden">
        {isVolumesLevel ? (
          <Button
            type="button"
            variant="secondary"
            size="sm"
            disabled={permissionChecking}
            onClick={onRequestPermissions}
          >
            <ShieldCheck data-icon="inline-start" />
            Permissions
          </Button>
        ) : null}
        <div className="min-w-0 flex-1 overflow-hidden">
          <PathButtonGroup
            path={currentPath}
            rootPath={selectedVolume?.mountPoint}
            disabled={isNavigationPending}
            onNavigate={(path) => void openPath(path)}
            onBackToRoot={returnToVolumes}
          />
        </div>
        <ButtonGroup>
          {canReloadCurrentPath ? (
            <Button
              type="button"
              variant="outline"
              size="icon-sm"
              aria-label="Reload current path"
              disabled={isReloadingCurrentPath || isNavigationPending}
              onClick={() => void reloadCurrentPath()}
            >
              <RefreshCwIcon data-icon="inline-start" />
            </Button>
          ) : null}
          <Button
            type="button"
            variant={visualizerOpen ? "secondary" : "outline"}
            size="icon-sm"
            aria-label={visualizerOpen ? "Hide visualizer" : "Show visualizer"}
            aria-pressed={visualizerOpen}
            onClick={onVisualizerToggle}
          >
            <PanelRightOpenIcon data-icon="inline-start" />
          </Button>
        </ButtonGroup>
      </div>
      {error ? <p className="text-sm text-destructive">{error}</p> : null}
      <ExplorerTable
        className={tableClassName}
        volumes={volumes}
        listing={listing}
        liveUpdates={listingLiveUpdates}
        loadingPath={loadingPath}
        busyOverlay={busyOverlay}
        selectedItemIds={selectedItemIds}
        selectionAnchorId={explorerSelectionAnchorId}
        onOpenVolume={openVolume}
        onOpenNode={openNode}
        onDeleteItems={deleteExplorerItems}
        onSelectionAnchorChange={onExplorerSelectionAnchorChange}
        onSelectionChange={onSelectionChange}
      />
      {selectedVolume ? <VolumeDetails volume={selectedVolume} /> : null}
    </section>
  )
}

function listingNeedsScan(listing: DirectoryListingDto) {
  return listing.children.some((node) => node.kind === "directory" && node.state !== "complete")
}

function isSameVolume(left: DriveDto, right: DriveDto) {
  return left.mountPoint === right.mountPoint || left.id === right.id
}

function scanKey(path: string) {
  return `${path}::${JSON.stringify(defaultScanConfig)}`
}

function progressEventKey(jobId: string, requestId: string) {
  return `${jobId}:${requestId}`
}

function volumeToVisualizerItem(volume: DriveDto): VisualizerCellInput {
  return {
    id: volume.id,
    label: volumeNameOnly(volume.label, volume.mountPoint),
    path: volume.mountPoint,
    kind: "volume",
    size: Math.max(0, volume.usedSpace),
    state: "complete",
  }
}

function nodeToVisualizerItem(node: PathNodeDto, update?: DirectoryProgressUpdateDto): VisualizerCellInput {
  return {
    id: node.path,
    label: node.name,
    path: node.path,
    kind: node.kind,
    size: Math.max(0, update?.logicalSize ?? node.logicalSize),
    state: update?.state ?? node.state,
  }
}

function createVisualizerGeneration(path: string | null, items: VisualizerCellInput[]) {
  return `${path ?? "volumes"}:${items.map((item) => `${item.id}:${item.size}:${item.state ?? ""}`).join("|")}`
}

function getVisualizerParentPath(path: string, rootPath: string) {
  const normalizedPath = normalizePath(path)
  const normalizedRoot = normalizePath(rootPath)

  if (normalizedPath === normalizedRoot || !isInsideRoot(normalizedPath, normalizedRoot)) {
    return null
  }

  const relative = normalizedPath.slice(normalizedRoot.length).replace(/^\/+/, "")
  const parts = relative.split("/").filter(Boolean)
  if (parts.length <= 1) {
    return normalizedRoot
  }

  const parentPath = parts.slice(0, -1).join("/")
  return normalizedRoot === "/" ? `/${parentPath}` : `${normalizedRoot}/${parentPath}`
}

function normalizePath(path: string) {
  const normalized = path.replaceAll("\\", "/").replace(/\/+$/, "")
  return normalized || "/"
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

function lastPathPart(path: string) {
  if (path === "/") {
    return "/"
  }

  return path.split("/").filter(Boolean).at(-1) ?? path
}

function isInsideRoot(path: string, rootPath: string) {
  return rootPath === "/"
    ? path.startsWith("/")
    : path === rootPath || path.startsWith(`${rootPath}/`)
}

function progressSnapshotToOverlayProps(progress: ScanProgressState | undefined, path: string) {
  if (!progress || progress.path !== path) {
    return {}
  }

  const { snapshot } = progress
  const totalBytes = snapshot.estimatedTotalBytes ?? 0
  const hasEstimate = totalBytes > 0
  if (!hasEstimate) {
    return {
      infinite: true,
    }
  }

  const percent = Math.min(snapshot.state === "completed" ? 100 : 99, (snapshot.bytesMeasured / totalBytes) * 100)

  return {
    infinite: false,
    progress: percent,
  }
}
