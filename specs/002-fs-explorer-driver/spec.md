# Feature Specification: Filesystem Explorer Driver

**Feature Branch**: `002-fs-explorer-driver`

**Created**: 2026-05-17

**Status**: Draft

**Input**: User description: "Create an fs-explorer component at the core of Space Lenser with a cross-platform driver for listing volumes, asynchronous scans, size-rule scan depth, threshold filtering, hidden-file visibility, lazy-loaded precise directory levels, accurate progress, non-blocking navigation, cached directories, path invalidation, breadcrumb-style grouped navigation, clickable directory rows, and per-item working indicators."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Browse Volumes Into Directories (Priority: P1)

A user opens the explorer, sees available volumes, chooses one, and navigates into folders using the visible path controls and clickable directory rows. Navigation must remain available even when some folders are still being measured.

**Why this priority**: Volume-to-directory browsing is the minimum useful explorer experience and the entry point for every later scan rule.

**Independent Test**: Can be tested by opening the explorer, selecting a mounted volume, clicking into at least three nested folders, and returning to a parent path without starting over.

**Acceptance Scenarios**:

1. **Given** the explorer has loaded available volumes, **When** the user selects a volume, **Then** the explorer shows that volume's immediate folders and files as navigable rows.
2. **Given** a directory listing is visible, **When** the user clicks a folder row, **Then** the explorer immediately navigates into that folder and shows cached or placeholder contents without blocking on unfinished measurements.
3. **Given** the user is inside a nested path, **When** the user selects a parent segment from the path controls, **Then** the explorer returns to that parent path without discarding already discovered child data.

---

### User Story 2 - Scan By Size Rules (Priority: P1)

A user configures scan behavior so large folders are explored deeper while small folders are removed or collapsed from the visible result set. The explorer repeats the rule at each discovered level.

**Why this priority**: The product must identify the folders that matter without scanning every file under every folder before the user can navigate.

**Independent Test**: Can be tested with a fixture tree containing folders above and below the configured thresholds and verifying that large folders expand deeper while small folders are excluded from the visible directory set.

**Acceptance Scenarios**:

1. **Given** a rule that expands folders larger than 1 GB by one more level, **When** a folder above that threshold is measured, **Then** the explorer schedules its next level and applies the same rule to that level.
2. **Given** a rule that removes folders smaller than 100 MB, **When** a folder below that threshold is measured, **Then** that folder is omitted from the visible directory rows unless the user changes the filtering configuration.
3. **Given** hidden files are disabled, **When** a directory contains hidden entries, **Then** those entries are omitted from the visible listing and their skipped status is reflected in the directory's scan metadata.

---

### User Story 3 - Lazy-Load Deep Levels (Priority: P2)

A user requests a deeper look from a selected path, such as ten levels, while the explorer preloads the first few levels and defers the rest until navigation or rules require them.

**Why this priority**: Deep trees must stay usable at scale without forcing a full-file traversal before the user can inspect high-value folders.

**Independent Test**: Can be tested by requesting a ten-level scan on a deep fixture tree and verifying the first two levels become usable quickly while deeper levels are scheduled and loaded as the user navigates.

**Acceptance Scenarios**:

1. **Given** the user requests ten levels with the first two preloaded, **When** the scan starts, **Then** the first two levels become available before lower-priority deeper jobs finish.
2. **Given** a deeper level is not yet loaded, **When** the user navigates toward it, **Then** the explorer shows the known path immediately and updates individual rows as their jobs complete.
3. **Given** a directory has been measured before, **When** the user returns to it, **Then** the explorer uses the cached result without re-running the measurement.

---

### User Story 4 - Track Accurate Progress And Working Items (Priority: P2)

A user watches scans progress while the explorer reports honest completion state for each active job and each visible directory row.

**Why this priority**: Long disk scans require trust. Users must know what is complete, what is still working, and where results may still change.

**Independent Test**: Can be tested with a large fixture tree by starting a scan, observing per-row working states, and verifying progress reaches complete only after all scheduled work has a terminal state.

**Acceptance Scenarios**:

1. **Given** one or more scan jobs are active, **When** the explorer displays progress, **Then** progress never claims completion until all scheduled jobs for the current request are complete, failed, skipped, or canceled.
2. **Given** a visible folder is still being measured, **When** the user views the table, **Then** that row shows a working indicator while still allowing navigation actions elsewhere.
3. **Given** the system discovers additional work during traversal, **When** progress is recalculated, **Then** the reported total adjusts without decreasing already completed work counts.

---

### User Story 5 - Invalidate And Rediscover A Path (Priority: P3)

A user refreshes a specific path after filesystem contents change, causing only that path and its affected descendants to be rediscovered.

**Why this priority**: Users need a way to correct stale results without discarding the entire explorer cache.

**Independent Test**: Can be tested by scanning a fixture path, changing one subtree, invalidating that path, and verifying only that path's result changes while unrelated cached directories remain available.

**Acceptance Scenarios**:

1. **Given** a directory is cached, **When** the user invalidates that directory, **Then** its cached listing and affected descendant measurements are marked stale.
2. **Given** a path is marked stale, **When** the explorer rediscover job completes, **Then** the visible row and any open view for that path update with the new size and contents.

### Edge Cases

- The selected volume disappears, is unmounted, or becomes unreadable while scan jobs are active.
- A directory contains hard links, sparse files, compressed files, symlinks, mount boundaries, unreadable entries, or permission-denied descendants.
- The requested path is deleted, renamed, or replaced between navigation and measurement.
- A folder's size crosses a configured threshold after invalidation.
- The user navigates repeatedly while jobs for previously visited paths are still active.
- Hidden entries are toggled after a path has already been cached.
- A scan job discovers more work than was known when progress was first displayed.
- Multiple jobs request overlapping paths at the same time.
- Very large directories contain more rows than can be comfortably rendered at once.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide a single explorer surface that starts at available volumes and navigates into directories and files from the currently selected path.
- **FR-002**: The system MUST separate the explorer experience from the filesystem analysis provider so volume listing, path discovery, measurement, progress, cache reads, and invalidation are exposed through a consistent product-level contract.
- **FR-003**: The system MUST list available volumes with enough information for users to distinguish them, including label or path, total capacity, available capacity, and accessibility status when known.
- **FR-004**: The system MUST allow users to navigate by grouped path controls and clickable directory rows; no additional navigation control type is required for v1.
- **FR-005**: The system MUST show directory contents immediately from cache, partial discovery, or placeholders while background jobs continue.
- **FR-006**: The system MUST never block path navigation because a scan or measurement job is still running.
- **FR-007**: The system MUST support scan configuration by requested start path, requested depth, preload depth, hidden-entry visibility, expansion threshold, and minimum visible folder threshold.
- **FR-008**: The system MUST support recursive size-rule expansion where a folder above the configured expansion threshold schedules one additional level and repeats the same rule at that next level.
- **FR-009**: The system MUST support removing or hiding folders below the configured minimum visible folder threshold from the visible result set.
- **FR-010**: The system MUST report exact measured size for every completed visible directory row according to the product's filesystem-size semantics.
- **FR-011**: The system MUST distinguish complete, working, queued, skipped, failed, stale, and partially known states for scan jobs and directory rows.
- **FR-012**: The system MUST display a working indicator for each visible item that is still being measured or refreshed.
- **FR-013**: The system MUST maintain a per-path directory cache so returning to a previously requested path does not trigger a new backend request unless that path is stale or explicitly invalidated.
- **FR-014**: The system MUST allow a specific path to be invalidated and rediscovered without clearing unrelated cached paths.
- **FR-015**: The system MUST surface access-denied paths, skipped hidden entries, skipped mount boundaries, and unreadable entries as scan metadata rather than silently dropping their impact.
- **FR-016**: The system MUST avoid double-counting filesystem content that appears through multiple directory entries when the underlying platform reports it as the same stored content.
- **FR-017**: The system MUST use platform-appropriate on-disk size semantics so reported totals reflect disk usage rather than only logical file length when the platform exposes that distinction.
- **FR-018**: The system MUST stream job progress updates while work is active and MUST update visible rows as soon as individual path results are available.
- **FR-019**: The system MUST report progress as a structured state that includes known completed work, known remaining work, discovered additional work, and terminal failures or skips.
- **FR-020**: The system MUST keep progress honest when the total amount of work is discovered during scanning; it MUST NOT present a fixed percentage as final until the request's scheduled work has reached terminal states.
- **FR-021**: The system MUST support concurrent scan jobs for different paths while preserving one authoritative cached result per path.
- **FR-022**: The system MUST provide cancellation or replacement semantics for stale jobs when a path is invalidated or a newer request supersedes an older one.
- **FR-023**: The system MUST preserve responsive interaction for volume selection, path navigation, row selection, and path invalidation while background jobs are active.
- **FR-024**: The system MUST bound v1 to browsing, scanning, progress, caching, and invalidation; file deletion, moving, renaming, and permissions management are out of scope.

### Key Entities

- **Filesystem Explorer**: The user-facing navigation surface for volumes, current path, directory rows, working states, and cached results.
- **Analysis Provider**: The product-level contract that lists volumes, schedules scan jobs, reports progress, returns directory listings, and invalidates paths.
- **Volume**: A mountable storage root with display identity, root path, capacity details, filesystem metadata, and accessibility state.
- **Path Node**: A file or directory entry with display name, full path, type, measured size, visibility state, scan state, and relationship to parent and child paths.
- **Scan Configuration**: User or product defaults controlling start path, requested depth, preload depth, hidden-entry visibility, expansion threshold, and minimum visible folder threshold.
- **Scan Job**: An asynchronous unit of work that discovers or refreshes a path and reports progress, terminal state, errors, and discovered follow-up work.
- **Directory Cache Entry**: Stored listing and measurement state for a path, including freshness, applied configuration, visible children, skipped metadata, and active job references.
- **Progress Snapshot**: A point-in-time report of completed work, known remaining work, newly discovered work, active item states, skipped paths, and failures.
- **Invalidation Request**: A request to mark a path and affected descendants stale and schedule rediscovery while preserving unrelated cache entries.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can open the explorer and see available volumes within 2 seconds on a typical desktop with mounted local volumes.
- **SC-002**: Users can navigate from a volume into three nested directory levels without waiting for any full-volume scan to complete.
- **SC-003**: For a configured scan with two preloaded levels, the first two levels become navigable within 5 seconds for a 10,000-entry fixture on target development hardware.
- **SC-004**: Completed visible directory sizes match a validated reference scan within 0.1% for normal files and exactly for hard-link deduplication cases in controlled fixtures.
- **SC-005**: Progress reaches 100% only after all jobs created for the active request have reached complete, skipped, failed, or canceled states.
- **SC-006**: At least 95% of row-level updates appear in the explorer within 500 ms after their corresponding job result is available.
- **SC-007**: Returning to a previously loaded path uses cached data without a new measurement request in 100% of cases unless the path is stale, invalidated, or requested with different scan-affecting configuration.
- **SC-008**: Invalidating one path refreshes that path and affected descendants while preserving unrelated cached paths in 100% of tested fixture cases.
- **SC-009**: Users can continue selecting path controls and directory rows while at least five scan jobs are active, with no interaction blocked for more than 100 ms by scan work.
- **SC-010**: Access-denied, skipped, hidden, and boundary-limited paths are visible as metadata in 100% of affected completed scan results.

## Assumptions

- The primary user is a single desktop user analyzing local or mounted storage from an interactive desktop application.
- The first product slice focuses on local filesystem exploration and does not include remote cloud storage.
- Default scan behavior preloads the first two levels when the user requests deeper lazy-loaded exploration.
- Default size-rule examples are an expansion threshold of folders larger than 1 GB and a minimum visible folder threshold of 100 MB, but both are configurable.
- Hidden entries are excluded by default unless the user or product configuration enables them.
- Symlinks are not followed by default, and crossing into a different mounted filesystem is treated as a boundary unless explicitly allowed by configuration.
- Progress accuracy is defined as honest accounting of known, completed, skipped, failed, and newly discovered work, rather than pretending the total cost is known before discovery.
- Implementation planning must validate platform-specific size semantics and filesystem identity behavior before coding the analysis provider.
