import { useCallback, useEffect, useMemo, useState } from "react"
import { RefreshCwIcon } from "lucide-react"

import { fsListVolumes, fsOpenPath, fsStartScan } from "@/api"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { ExplorerTable } from "./ExplorerTable"
import { PathButtonGroup } from "./PathButtonGroup"
import { createExplorerCache, getCachedListing, putCachedListing } from "./cache"
import type {
  DirectoryListingDto,
  DriveDto,
  ExplorerCache,
  FsProgressEvent,
  PathNodeDto,
} from "./types"
import { defaultScanConfig } from "./types"

export function FsExplorer() {
  const [volumes, setVolumes] = useState<DriveDto[]>([])
  const [selectedVolume, setSelectedVolume] = useState<DriveDto>()
  const [currentPath, setCurrentPath] = useState<string>()
  const [cache, setCache] = useState<ExplorerCache>(() => createExplorerCache())
  const [loadingPath, setLoadingPath] = useState<string>()
  const [error, setError] = useState<string>()

  const listing = useMemo(
    () => (currentPath ? getCachedListing(cache, currentPath) : undefined),
    [cache, currentPath],
  )
  const canReloadCurrentPath = Boolean(selectedVolume && currentPath)
  const isReloadingCurrentPath = Boolean(currentPath && loadingPath === currentPath)

  useEffect(() => {
    fsListVolumes()
      .then(setVolumes)
      .catch((error: unknown) => setError(error instanceof Error ? error.message : String(error)))
  }, [])

  const mergeListing = useCallback((listing: DirectoryListingDto) => {
    setCache((cache) => putCachedListing(cache, listing))
  }, [])

  const handleProgress = useCallback(
    (event: FsProgressEvent) => {
      if (event.event === "directoryReady") {
        mergeListing(event.data.listing)
      }
      if (event.event === "jobFinished" || event.event === "jobFailed") {
        setLoadingPath(undefined)
      }
    },
    [mergeListing],
  )

  const openPath = useCallback(
    async (path: string, volume = selectedVolume) => {
      if (!volume) {
        return
      }
      const volumeRoot = volume.mountPoint
      setCurrentPath(path)
      const cached = getCachedListing(cache, path)
      if (cached) return

      setLoadingPath(path)
      try {
        const listing = await fsOpenPath(path, volumeRoot, defaultScanConfig)
        mergeListing(listing)
        if (listingNeedsScan(listing)) {
          await fsStartScan(path, volumeRoot, defaultScanConfig, handleProgress)
        } else {
          setLoadingPath(undefined)
        }
      } catch (error) {
        setError(error instanceof Error ? error.message : String(error))
        setLoadingPath(undefined)
      }
    },
    [cache, handleProgress, mergeListing, selectedVolume],
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
    try {
      await fsStartScan(currentPath, selectedVolume.mountPoint, defaultScanConfig, handleProgress)
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error))
      setLoadingPath(undefined)
    }
  }, [currentPath, handleProgress, selectedVolume])

  const returnToVolumes = () => {
    setSelectedVolume(undefined)
    setCurrentPath(undefined)
    setLoadingPath(undefined)
    setError(undefined)
  }

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex min-h-8 items-center gap-2 overflow-hidden">
        <div className="min-w-0 flex-1 overflow-hidden">
          <PathButtonGroup
            path={currentPath}
            rootPath={selectedVolume?.mountPoint}
            rootLabel={selectedVolume?.label}
            onNavigate={(path) => void openPath(path)}
            onBackToRoot={returnToVolumes}
          />
        </div>
        {canReloadCurrentPath ? (
          <Button
            type="button"
            variant="outline"
            size="icon-sm"
            aria-label="Reload current path"
            disabled={isReloadingCurrentPath}
            onClick={() => void reloadCurrentPath()}
          >
            {isReloadingCurrentPath ? (
              <Spinner />
            ) : (
              <RefreshCwIcon data-icon="inline-start" />
            )}
          </Button>
        ) : null}
      </div>
      {error ? <p className="text-sm text-destructive">{error}</p> : null}
      <ExplorerTable
        volumes={volumes}
        listing={listing}
        loadingPath={loadingPath}
        onOpenVolume={openVolume}
        onOpenNode={openNode}
      />
    </section>
  )
}

function listingNeedsScan(listing: DirectoryListingDto) {
  return listing.children.some((node) => node.kind === "directory" && node.state !== "complete")
}
