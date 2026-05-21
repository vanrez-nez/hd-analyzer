import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { PanelRightOpenIcon, RefreshCwIcon, ShieldCheck } from "lucide-react"

import { deleteItems, fsInvalidatePath, fsListVolumes, fsOpenPathWithProgress, fsStartScan } from "@/api"
import { ButtonGroup } from "@/components/ui/button-group"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import { ExplorerTable } from "./ExplorerTable"
import type { ExplorerLoadingOverlayProps } from "./ExplorerLoadingOverlay"
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
type PendingNavigationUpdate =
  | PendingNavigation
  | undefined
  | ((pending: PendingNavigation | undefined) => PendingNavigation | undefined)
type ScanProgressState = {
  path: string
  snapshot: ProgressSnapshotDto
}
type CompletionOverlay = ExplorerLoadingOverlayProps & {
  path: string
}
type ProgressOverlayMode = "completed" | "determinate" | "indeterminate"

const SCAN_COMPLETION_OVERLAY_MS = 300

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
  const [isReloadingVolumes, setIsReloadingVolumes] = useState(false)
  const [pendingNavigation, setPendingNavigation] = useState<PendingNavigation>()
  const [completionOverlay, setCompletionOverlay] = useState<CompletionOverlay>()
  const [scanProgress, setScanProgress] = useState<ScanProgressState>()
  const [liveUpdates, setLiveUpdates] = useState<LiveDirectoryUpdates>({})
  const [error, setError] = useState<string>()
  const activeScansRef = useRef<Set<string>>(new Set())
  const completionOverlayTimeoutRef = useRef<number | undefined>(undefined)
  const pendingNavigationRef = useRef<PendingNavigation | undefined>(undefined)
  const scanProgressByPathRef = useRef<Map<string, ProgressSnapshotDto>>(new Map())
  const scanJobPathRef = useRef<Map<string, string>>(new Map())
  const navigationRequestRef = useRef(0)

  const listing = useMemo(
    () => (currentPath ? getCachedListing(cache, currentPath) : undefined),
    [cache, currentPath],
  )
  const listingLiveUpdates = listing ? liveUpdates[listing.path] : undefined
  const visualizerLiveUpdates = visualizerOpen ? listingLiveUpdates : undefined
  const canReloadCurrentPath = Boolean(selectedVolume && currentPath)
  const isNavigationPending = Boolean(pendingNavigation || completionOverlay)
  const isReloadingCurrentPath = Boolean(currentPath && loadingPath === currentPath)
  const isVolumesLevel = !currentPath
  const reloadDisabled = isVolumesLevel ? isReloadingVolumes : isReloadingCurrentPath || isNavigationPending
  const busyOverlay = useMemo(() => {
    if (!pendingNavigation) {
      return completionOverlay
    }

    return {
      ...pendingNavigation,
      infinite: true,
      operationKey: `${pendingNavigation.phase}:${pendingNavigation.path}`,
      ...progressSnapshotToOverlayProps(scanProgress, pendingNavigation.path),
    }
  }, [completionOverlay, pendingNavigation, scanProgress])

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

  const clearCompletionOverlay = useCallback(() => {
    if (completionOverlayTimeoutRef.current !== undefined) {
      window.clearTimeout(completionOverlayTimeoutRef.current)
      completionOverlayTimeoutRef.current = undefined
    }
    setCompletionOverlay(undefined)
  }, [])

  const setPendingNavigationState = useCallback((update: PendingNavigationUpdate) => {
    setPendingNavigation((current) => {
      const next = typeof update === "function" ? update(current) : update
      pendingNavigationRef.current = next
      return next
    })
  }, [])

  const showCompletionOverlay = useCallback((path: string, jobId: string, requestId: string) => {
    if (completionOverlayTimeoutRef.current !== undefined) {
      window.clearTimeout(completionOverlayTimeoutRef.current)
    }

    const snapshot = scanProgressByPathRef.current.get(path)
    const operationKey = `completed:${path}:${jobId}:${requestId}`
    setCompletionOverlay({
      entriesProcessed: snapshot?.completedUnits ?? 0,
      forceVisible: true,
      infinite: false,
      operationKey,
      path,
      phase: "scanning",
      progress: 100,
    })

    completionOverlayTimeoutRef.current = window.setTimeout(() => {
      setCompletionOverlay((overlay) => (overlay?.operationKey === operationKey ? undefined : overlay))
      completionOverlayTimeoutRef.current = undefined
    }, SCAN_COMPLETION_OVERLAY_MS)
  }, [])

  useEffect(() => {
    refreshVolumes().catch((error: unknown) => setError(error instanceof Error ? error.message : String(error)))
  }, [currentPath, refreshVolumes])

  useEffect(() => {
    pendingNavigationRef.current = pendingNavigation
  }, [pendingNavigation])

  useEffect(
    () => () => {
      if (completionOverlayTimeoutRef.current !== undefined) {
        window.clearTimeout(completionOverlayTimeoutRef.current)
      }
    },
    [],
  )

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
          scanProgressByPathRef.current.set(progressPath, event.data.snapshot)
          setScanProgress({ path: progressPath, snapshot: event.data.snapshot })
        }
        setPendingNavigationState((pending) =>
          pending && progressPath === pending.path
            ? { ...pending, entriesProcessed: event.data.snapshot.completedUnits, phase: "scanning" }
            : pending,
        )
      }
      if (event.event === "directoryProgress") {
        scanJobPathRef.current.set(progressEventKey(event.data.jobId, event.data.requestId), event.data.path)
        scanProgressByPathRef.current.set(event.data.path, event.data.snapshot)
        setScanProgress({ path: event.data.path, snapshot: event.data.snapshot })
        setPendingNavigationState((pending) =>
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
        if (event.event === "jobFinished" && pendingNavigationRef.current?.path === event.data.path) {
          showCompletionOverlay(event.data.path, event.data.jobId, event.data.requestId)
        }
        setLoadingPath((loadingPath) => (loadingPath === event.data.path ? undefined : loadingPath))
        setScanProgress((progress) => (progress?.path === event.data.path ? undefined : progress))
        setPendingNavigationState((pending) => (pending?.path === event.data.path ? undefined : pending))
        scanProgressByPathRef.current.delete(event.data.path)
        setLiveUpdates((updates) => {
          if (!updates[event.data.path]) {
            return updates
          }

          const { [event.data.path]: _finished, ...remaining } = updates
          return remaining
        })
      }
    },
    [mergeListing, setPendingNavigationState, showCompletionOverlay],
  )

  const startScan = useCallback(
    async (path: string, volumeRoot: string, replaceExisting = false) => {
      const key = scanKey(path)
      if (!replaceExisting && activeScansRef.current.has(key)) {
        return false
      }

      activeScansRef.current.add(key)
      clearCompletionOverlay()
      const initialSnapshot = {
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
      } satisfies ProgressSnapshotDto
      scanProgressByPathRef.current.set(path, initialSnapshot)
      setScanProgress({
        path,
        snapshot: initialSnapshot,
      })
      try {
        await fsStartScan(path, volumeRoot, defaultScanConfig, handleProgress, replaceExisting)
        return true
      } catch (error) {
        activeScansRef.current.delete(key)
        scanProgressByPathRef.current.delete(path)
        throw error
      }
    },
    [clearCompletionOverlay, handleProgress],
  )

  const openPath = useCallback(
    async (path: string, volume = selectedVolume, forceRefresh = false) => {
      if (!volume) {
        return
      }
      clearCompletionOverlay()
      const volumeRoot = volume.mountPoint
      const cached = getCachedListing(cache, path)
      if (cached && !forceRefresh) {
        navigationRequestRef.current += 1
        setCurrentPath(cached.path)
        setPendingNavigationState(undefined)
        setLoadingPath(undefined)
        setError(undefined)
        return
      }

      const requestId = navigationRequestRef.current + 1
      navigationRequestRef.current = requestId
      setError(undefined)
      setLoadingPath(path)
      setPendingNavigationState({ entriesProcessed: 0, path, phase: "opening" })
      try {
        const listing = await fsOpenPathWithProgress(path, volumeRoot, defaultScanConfig, (progress) => {
          if (navigationRequestRef.current !== requestId) {
            return
          }

          setPendingNavigationState((pending) =>
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
          setPendingNavigationState({ entriesProcessed: 0, path: listing.path, phase: "scanning" })
          const started = await startScan(listing.path, volumeRoot)
          if (!started) {
            setPendingNavigationState(undefined)
            setLoadingPath(undefined)
          }
        } else {
          setPendingNavigationState(undefined)
          setLoadingPath(undefined)
        }
      } catch (error) {
        if (navigationRequestRef.current !== requestId) {
          return
        }

        setError(error instanceof Error ? error.message : String(error))
        setPendingNavigationState(undefined)
        setLoadingPath(undefined)
      }
    },
    [cache, clearCompletionOverlay, mergeListing, selectedVolume, setPendingNavigationState, startScan],
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
    setPendingNavigationState({ entriesProcessed: 0, path: currentPath, phase: "scanning" })
    try {
      const started = await startScan(currentPath, selectedVolume.mountPoint, true)
      if (!started) {
        setPendingNavigationState(undefined)
        setLoadingPath(undefined)
      }
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error))
      setPendingNavigationState(undefined)
      setLoadingPath(undefined)
    }
  }, [currentPath, selectedVolume, setPendingNavigationState, startScan])

  const reloadVolumes = useCallback(async () => {
    setError(undefined)
    clearCompletionOverlay()
    setIsReloadingVolumes(true)
    try {
      await refreshVolumes()
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error))
    } finally {
      setIsReloadingVolumes(false)
    }
  }, [clearCompletionOverlay, refreshVolumes])

  const returnToVolumes = () => {
    navigationRequestRef.current += 1
    clearCompletionOverlay()
    setSelectedVolume(undefined)
    setCurrentPath(undefined)
    setLoadingPath(undefined)
    setPendingNavigationState(undefined)
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
        <TooltipProvider delayDuration={250}>
          <ButtonGroup>
            {isVolumesLevel || canReloadCurrentPath ? (
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="outline"
                    size="icon-sm"
                    aria-label={isVolumesLevel ? "Reload volumes" : "Reload current path"}
                    disabled={reloadDisabled}
                    onClick={() => void (isVolumesLevel ? reloadVolumes() : reloadCurrentPath())}
                  >
                    <RefreshCwIcon data-icon="inline-start" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom">
                  {isVolumesLevel ? "Reload volumes" : "Reload current path"}
                </TooltipContent>
              </Tooltip>
            ) : null}
            <Tooltip>
              <TooltipTrigger asChild>
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
              </TooltipTrigger>
              <TooltipContent side="bottom">
                {visualizerOpen ? "Hide visualizer" : "Show visualizer"}
              </TooltipContent>
            </Tooltip>
          </ButtonGroup>
        </TooltipProvider>
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
  const mode = progressOverlayMode(snapshot)
  if (mode === "completed") {
    return {
      infinite: false,
      progress: 100,
    }
  }

  if (mode === "indeterminate") {
    return {
      infinite: true,
    }
  }

  const totalBytes = snapshot.estimatedTotalBytes ?? 0
  const percent = Math.min(99, (snapshot.bytesMeasured / totalBytes) * 100)

  return {
    infinite: false,
    progress: percent,
  }
}

function progressOverlayMode(snapshot: ProgressSnapshotDto): ProgressOverlayMode {
  if (snapshot.state === "completed") {
    return "completed"
  }

  const totalBytes = snapshot.estimatedTotalBytes ?? 0
  return totalBytes > 0 && snapshot.bytesMeasured <= totalBytes ? "determinate" : "indeterminate"
}
