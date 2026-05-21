import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { PanelRightOpenIcon, RefreshCwIcon, ShieldCheck } from "lucide-react"

import { fsListVolumes, fsOpenPathWithProgress, fsStartScan } from "@/api"
import { ButtonGroup } from "@/components/ui/button-group"
import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import { ExplorerTable } from "./ExplorerTable"
import { PathButtonGroup } from "./PathButtonGroup"
import { createExplorerCache, getCachedListing, putCachedListing } from "./cache"
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
  const [scanProgress, setScanProgress] = useState<ProgressSnapshotDto>()
  const [liveUpdates, setLiveUpdates] = useState<LiveDirectoryUpdates>({})
  const [error, setError] = useState<string>()
  const activeScansRef = useRef<Set<string>>(new Set())
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

  useEffect(() => {
    fsListVolumes()
      .then(setVolumes)
      .catch((error: unknown) => setError(error instanceof Error ? error.message : String(error)))
  }, [])

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
      if (event.event === "directoryReady") {
        mergeListing(event.data.listing)
      }
      if (event.event === "progressSnapshot") {
        setScanProgress(event.data.snapshot)
        setPendingNavigation((pending) =>
          pending && scanSnapshotMatchesPath(event.data.snapshot, pending.path)
            ? { ...pending, entriesProcessed: event.data.snapshot.completedUnits, phase: "scanning" }
            : pending,
        )
      }
      if (event.event === "directoryProgress") {
        setScanProgress(event.data.snapshot)
        setPendingNavigation((pending) =>
          pending && (event.data.path === pending.path || scanSnapshotMatchesPath(event.data.snapshot, pending.path))
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
        activeScansRef.current.delete(scanKey(event.data.path))
        setLoadingPath((loadingPath) => (loadingPath === event.data.path ? undefined : loadingPath))
        setScanProgress(undefined)
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
    async (path: string, volume = selectedVolume) => {
      if (!volume) {
        return
      }
      const volumeRoot = volume.mountPoint
      const cached = getCachedListing(cache, path)
      if (cached) {
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
            rootLabel={selectedVolume?.label}
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
      {scanProgress ? <ScanProgressBar snapshot={scanProgress} /> : null}
      {error ? <p className="text-sm text-destructive">{error}</p> : null}
      <ExplorerTable
        className={tableClassName}
        volumes={volumes}
        listing={listing}
        liveUpdates={listingLiveUpdates}
        loadingPath={loadingPath}
        busyOverlay={pendingNavigation}
        selectedItemIds={selectedItemIds}
        selectionAnchorId={explorerSelectionAnchorId}
        onOpenVolume={openVolume}
        onOpenNode={openNode}
        onSelectionAnchorChange={onExplorerSelectionAnchorChange}
        onSelectionChange={onSelectionChange}
      />
    </section>
  )
}

function listingNeedsScan(listing: DirectoryListingDto) {
  return listing.children.some((node) => node.kind === "directory" && node.state !== "complete")
}

function scanKey(path: string) {
  return `${path}::${JSON.stringify(defaultScanConfig)}`
}

function scanSnapshotMatchesPath(snapshot: ProgressSnapshotDto, path: string) {
  return snapshot.activePaths.includes(path)
}

function volumeToVisualizerItem(volume: DriveDto): VisualizerCellInput {
  return {
    id: volume.id,
    label: volume.label,
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

function isInsideRoot(path: string, rootPath: string) {
  return rootPath === "/"
    ? path.startsWith("/")
    : path === rootPath || path.startsWith(`${rootPath}/`)
}

function ScanProgressBar({ snapshot }: { snapshot: ProgressSnapshotDto }) {
  const totalBytes = snapshot.estimatedTotalBytes ?? 0
  const hasEstimate = totalBytes > 0
  const percent = hasEstimate
    ? Math.min(snapshot.state === "completed" ? 100 : 99, (snapshot.bytesMeasured / totalBytes) * 100)
    : 0

  return (
    <div className="flex h-5 items-center gap-2">
      <Progress value={percent} className="h-1.5 flex-1" />
      <span className="w-12 shrink-0 text-right text-xs tabular-nums text-muted-foreground">
        {hasEstimate ? `${Math.round(percent)}%` : "Scan"}
      </span>
    </div>
  )
}
