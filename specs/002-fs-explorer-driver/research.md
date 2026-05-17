# Research: Filesystem Explorer Driver

## Decision: Use `jwalk` as the traversal engine

**Rationale**: `jwalk` provides parallel directory walking, max-depth/min-depth controls, hidden-entry skipping, symlink controls, and `process_read_dir` hooks that fit rule-driven discovery. It lets the driver schedule bounded path jobs instead of forcing a full-tree scan before navigation.

**Alternatives considered**:

- Current `rayon` recursion in `scan.rs`: useful history, but it returns a full result and does not expose the right hooks for cached lazy path jobs.
- `walkdir`: mature and simple, but sequential traversal would require more custom scheduling for the same behavior.
- Full manual traversal: maximum control, but more error-prone around concurrency, cancellation, and platform behavior.

Sources: <https://docs.rs/jwalk/latest/jwalk/struct.WalkDirGeneric.html>

## Decision: Make on-disk size semantics part of the core driver

**Rationale**: Size correctness is product behavior, not a UI detail. Unix must use allocated blocks from `std::os::unix::fs::MetadataExt` and dedupe hard links globally by `(dev, ino)` when the filesystem reports multiple links. Windows must detect sparse/compressed attributes and use `GetCompressedFileSizeW` through the `windows` crate to report allocated size where available. Volume capacity remains backed by `sysinfo`.

**Alternatives considered**:

- `metadata().len()` everywhere: simpler but wrong for hard links, sparse files, and compressed files.
- Per-directory hard-link dedupe only: still double-counts links that appear in separate branches of the same scan.
- Shelling out to platform tools: difficult to make portable, testable, and responsive inside Tauri jobs.

Sources: <https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getcompressedfilesizew>, <https://docs.rs/sysinfo/latest/sysinfo/struct.Disk.html>

## Decision: Represent progress as discovered work state, not only a percentage

**Rationale**: The scanner cannot know the total folder count or total size before it discovers the tree. Progress must therefore expose `scheduled`, `discovered`, `completed`, `skipped`, `failed`, `canceled`, and `active` counts. A percentage can be derived only from known scheduled work and must reach complete only after all work spawned for the request reaches a terminal state.

**Alternatives considered**:

- Fixed 0-100 progress from root entry count: can be useful visually, but it becomes misleading when traversal discovers more work below large folders.
- Byte-based progress from total path size: circular for this feature because total path size is the result of the scan.

## Decision: Cache by path plus scan-affecting configuration

**Rationale**: Navigation must be instant when returning to known paths, but entries must also be invalidated when hidden visibility, thresholds, preload depth, or measurement rules change. The cache key uses canonical path identity plus a fingerprint of scan-affecting configuration. A path invalidation marks the path and affected descendants stale while preserving unrelated entries.

**Alternatives considered**:

- Cache by path only: incorrect when the same path is requested with different hidden or threshold rules.
- Global tree cache only: difficult to invalidate one path and awkward for concurrent jobs on different paths.

## Decision: Stream Tauri progress over channels and keep commands request/response oriented

**Rationale**: Tauri v2 channels are documented for streaming data from Rust to the frontend. Commands should start jobs, request cached listings, invalidate paths, and cancel jobs; channels should carry row updates, directory-ready events, and progress snapshots. JSON remains acceptable for control payloads. Bulk directory snapshots must be bounded and format-neutral so `rmp-serde` can be introduced if profiling proves JSON IPC is the bottleneck.

**Alternatives considered**:

- Polling with repeated `invoke`: increases frontend complexity and delays row updates.
- Tauri events for all updates: viable for small payloads, but channels give a clearer request-owned stream for scan jobs.
- MessagePack for all IPC immediately: adds surface area before there is a measured bottleneck; better to design DTOs so the transport can change.

Sources: <https://v2.tauri.app/es/develop/calling-rust/>, <https://v2.tauri.app/fr/develop/calling-frontend/>

## Decision: Use shadcn ButtonGroup, Table, and Spinner for the explorer surface

**Rationale**: The user requested grouped path navigation, clickable table rows, and per-item working indicators. shadcn provides the needed primitives without adding unrelated controls. The v1 explorer will keep navigation to `ButtonGroup` path segments and table row clicks, with the existing Permissions button left outside the explorer.

**Alternatives considered**:

- Tree view: familiar for file browsers, but the user explicitly requested table navigation and lazy row updates.
- Tabs/sidebar controls: unnecessary for v1 and conflicts with the request to avoid extra controls.

Sources: <https://ui.shadcn.com/docs/components/radix/button-group>, <https://ui.shadcn.com/docs/components/base/table>, <https://ui.shadcn.com/docs/components/base/spinner>

## Decision: Carry forward Spaceman's live-tree lessons, not its exact scan model

**Rationale**: `external/spaceman` demonstrates useful scan handles, live tree updates, chunked insertion, parent/child node relationships, and Unix filesystem-boundary checks. The fs-explorer needs stronger contracts around progress, size correctness, per-path cache entries, stale state, and lazy navigation, so Spaceman is a reference implementation rather than an architecture to copy directly.

**Alternatives considered**:

- Port Spaceman wholesale: faster initially, but it does not meet the core correctness and cache requirements.
- Ignore Spaceman: would discard useful traversal and live-update patterns already validated in a related project.
