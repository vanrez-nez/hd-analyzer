# Data Model: Tauri Desktop Migration

## Drive

Represents a selectable mounted storage volume.

**Fields**:

- `id`: stable UI identifier derived from mount point
- `label`: display label
- `mountPoint`: absolute path
- `totalSpace`: bytes
- `availableSpace`: bytes
- `usedSpace`: bytes
- `fileSystem`: display filesystem name

**Validation Rules**:

- `totalSpace` MUST be greater than zero.
- `usedSpace` MUST be `totalSpace - availableSpace` with saturating arithmetic.
- Duplicate mount points MUST collapse to one drive.

## ScanSession

Represents the active or completed scan for one root path.

**Fields**:

- `sessionId`: opaque ID assigned by the Tauri backend
- `root`: scanned root path
- `status`: `idle | scanning | complete | failed | partial_rescan`
- `startedAt`: backend timestamp
- `completedAt`: optional backend timestamp
- `progress`: latest `ScanProgress`
- `result`: optional `ScanResult`
- `focusedRescanPath`: optional path for partial rescan
- `errorMessage`: optional failure text

**State Transitions**:

- `idle -> scanning` when a user starts a scan.
- `scanning -> complete` when full scan result is available.
- `scanning -> failed` when scan startup or traversal fails fatally.
- `complete -> partial_rescan` when a focused subtree rescan starts.
- `partial_rescan -> complete` when subtree totals merge back into the result.

## ScanProgress

Incremental scan progress emitted while work is running.

**Fields**:

- `bytesScanned`: allocated bytes counted so far
- `filesScanned`: file count
- `directoriesScanned`: directory count
- `directorySizes`: optional snapshot map for progressive display
- `categories`: optional category snapshot

**Validation Rules**:

- Counts MUST be monotonically non-decreasing within one full scan.
- Snapshot totals MUST use the same allocated-size logic as final results.

## ScanResult

Final or current scan model used by desktop result views.

**Fields**:

- `root`: scanned root path
- `bytesScanned`: allocated bytes scanned
- `filesScanned`: file count
- `directoriesScanned`: directory count
- `directorySizes`: map of absolute directory path to allocated bytes
- `categories`: ordered `CategoryUsage` list
- `readErrors`: `ReadError` list
- `hiddenUnscannedBytes`: optional derived total at root

**Validation Rules**:

- `directorySizes[root]` MUST exist for completed scans.
- `categories` MUST include all known file kinds, with zero-size categories allowed internally.
- `hiddenUnscannedBytes` MUST NOT be mixed into scanned directory totals.

## DirectoryEntry

Represents one browsable row in the desktop explorer.

**Fields**:

- `path`: absolute path or virtual hidden-space identifier
- `displayName`: display name
- `size`: allocated bytes
- `share`: percentage of current result total
- `isDirectory`: boolean
- `isVirtual`: boolean
- `kind`: `directory | hidden_unscanned`

**Validation Rules**:

- Virtual hidden/unscanned rows MUST open the error log rather than filesystem traversal.
- Empty directories MAY be displayed but MUST NOT crash navigation.

## CategoryUsage

Represents a file-kind distribution segment.

**Fields**:

- `kind`: `video | audio | images | archives | apps | documents | code | other`
- `label`: display label
- `size`: allocated bytes
- `share`: percentage of scan total

**Validation Rules**:

- `share` MUST be derived from `size / ScanResult.bytesScanned`.
- Unknown extensions MUST map to `other` unless executable heuristics classify them as `apps`.

## ReadError

Represents unreadable filesystem content.

**Fields**:

- `path`: absolute path
- `displayPath`: path compacted relative to scan root
- `error`: message from filesystem operation

**Validation Rules**:

- Read errors MUST remain attached to their scan result.
- Error display MUST not imply the path was scanned successfully.

## UiState

Frontend state derived from backend snapshots and user navigation.

**Fields**:

- `screen`: `drive_selection | results | error_log`
- `selectedDriveId`: optional drive ID
- `currentPath`: optional absolute path
- `selectedEntryPath`: optional path
- `scanSessionId`: optional active session ID
- `lastError`: optional user-visible error

**Validation Rules**:

- UI state MUST tolerate backend scan progress arriving while the user navigates.
- The frontend MUST treat backend scan data as authoritative.

## ComponentSurface

Represents the required shadcn/ui-backed desktop component coverage.

**Fields**:

- `surface`: `drive_selection | scan_explorer | category_distribution | error_log | dialogs`
- `components`: shadcn/ui component names used by the surface
- `keyboardPaths`: primary keyboard interactions supported by the surface
- `density`: `compact | standard`, based on the amount of scan data shown
- `statusStates`: loading, scanning, complete, empty, error, and partial-rescan states visible on
  the surface

**Validation Rules**:

- Primary actions MUST use shared shadcn/ui button patterns.
- Dense directory and category views MUST use table/list primitives with stable row sizing.
- Dialog, tooltip, progress, badge, alert, and tab behavior MUST be consistent across screens.
- Focus states MUST be visible for keyboard navigation.
