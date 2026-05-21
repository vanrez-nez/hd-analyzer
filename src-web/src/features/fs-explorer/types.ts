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
  storageKind: string
  isRemovable: boolean
  isReadOnly: boolean
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
  sizeMeasurementMode?: "logicalOnly" | "logicalAndAllocated"
  liveUpdates?: LiveUpdateConfigDto
}

export type LiveUpdateConfigDto = {
  enabled: boolean
  throttleMs: number
  minBytesDelta: number
  maxBatchSize: number
}

export type ScanIssueDto = {
  path: string
  kind: string
  message: string
}

export type DeleteSafetyClassification =
  | "user_content"
  | "safe_junk"
  | "review_required"
  | "protected_system"
  | "not_deletable_now"

export type DeleteSafetyFlag =
  | "system_owned"
  | "sip_protected"
  | "immutable"
  | "append_only"
  | "parent_not_writable"
  | "not_deletable_now"
  | "safe_junk_rule"

export type PathDeleteSafetyDto = {
  classification: DeleteSafetyClassification
  canDeleteNow: boolean
  flags: DeleteSafetyFlag[]
  reason: string
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
  deleteSafety?: PathDeleteSafetyDto
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

export type FsOpenProgressEvent = {
  path: string
  entriesProcessed: number
}

export type ProgressSnapshotDto = {
  jobId: string
  requestId: string
  state: string
  estimatedTotalBytes?: number | null
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

export type DirectoryProgressUpdateDto = {
  path: string
  size: number
  logicalSize: number
  state: NodeState
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
  | {
      event: "directoryProgress"
      data: {
        jobId: string
        requestId: string
        path: string
        updates: DirectoryProgressUpdateDto[]
        snapshot: ProgressSnapshotDto
      }
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

export type DeleteReceiptDto = {
  deletedPaths: string[]
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
  sizeMeasurementMode: "logicalOnly",
  liveUpdates: {
    enabled: true,
    throttleMs: 250,
    minBytesDelta: 8_000_000,
    maxBatchSize: 128,
  },
}
