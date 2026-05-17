export type EntryKind = "volume" | "directory" | "file" | "other"
export type NodeState =
  | "queued"
  | "working"
  | "partial"
  | "complete"
  | "skipped"
  | "failed"
  | "stale"
  | "canceled"

export type DriveDto = {
  id: string
  label: string
  mountPoint: string
  totalSpace: number
  availableSpace: number
  usedSpace: number
  fileSystem: string
}

export type ScanConfigDto = {
  requestedDepth?: number
  preloadDepth?: number
  showHidden?: boolean
  expandAboveBytes?: number
  minVisibleFolderBytes?: number | null
  stayOnFilesystem?: boolean
  followSymlinks?: boolean
  dedupeHardLinks?: boolean
}

export type ScanIssueDto = {
  path: string
  kind: string
  message: string
}

export type PathNodeDto = {
  path: string
  name: string
  kind: EntryKind
  parentPath?: string | null
  depthFromRequest: number
  size: number
  logicalSize: number
  state: NodeState
  visible: boolean
  childrenKnown: boolean
  activeJobId?: string | null
  issues: ScanIssueDto[]
}

export type DirectoryListingDto = {
  path: string
  configFingerprint: string
  children: PathNodeDto[]
  totalVisibleSize: number
  totalMeasuredSize: number
  totalLogicalSize: number
  state: NodeState
  loadedDepth: number
  hasMoreDepth: boolean
  issues: ScanIssueDto[]
  generation: number
}

export type ProgressSnapshotDto = {
  jobId: string
  requestId: string
  state: string
  scheduledUnits: number
  discoveredUnits: number
  completedUnits: number
  activeUnits: number
  skippedUnits: number
  failedUnits: number
  canceledUnits: number
  bytesMeasured: number
  activePaths: string[]
}

export type FsProgressEvent =
  | { event: "jobQueued"; data: { jobId: string; requestId: string; path: string } }
  | { event: "jobStarted"; data: { jobId: string; requestId: string; path: string } }
  | {
      event: "directoryReady"
      data: { jobId: string; requestId: string; path: string; listing: DirectoryListingDto }
    }
  | {
      event: "progressSnapshot"
      data: { jobId: string; requestId: string; snapshot: ProgressSnapshotDto }
    }
  | { event: "jobFinished"; data: { jobId: string; requestId: string; path: string } }
  | { event: "jobFailed"; data: { jobId: string; requestId: string; path: string; message: string } }
  | {
      event: "pathInvalidated"
      data: { path: string; scope: "path_only" | "path_and_descendants"; generation: number }
    }

export type StartScanReceiptDto = {
  jobId: string
  requestId: string
}

export type InvalidationReceiptDto = {
  invalidatedPaths: string[]
  supersededJobIds: string[]
}

export type ExplorerCache = {
  listings: Record<string, DirectoryListingDto>
}

export const defaultScanConfig: ScanConfigDto = {
  requestedDepth: 2,
  preloadDepth: 1,
  showHidden: false,
  expandAboveBytes: 1_000_000_000,
  minVisibleFolderBytes: null,
  stayOnFilesystem: true,
  followSymlinks: false,
  dedupeHardLinks: false,
}
