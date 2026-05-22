# Tasks: Filesystem Explorer Driver

**Input**: Design documents from `/specs/002-fs-explorer-driver/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Include focused Rust tests for size calculations, hard-link dedupe, scan rules, cache invalidation, progress state, and driver behavior. Frontend validation uses typecheck/build plus manual desktop checks from quickstart.

**Organization**: Tasks are grouped by user story so each story can be implemented and tested as an independently valuable increment.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add dependencies, UI primitives, and empty module structure needed by the feature.

- [X] T001 Update workspace dependencies for `jwalk`, `thiserror`, `nix`, target-specific `windows`, and optional `rmp-serde` in Cargo.toml
- [X] T002 Update core crate dependencies for `jwalk`, `thiserror`, `nix`, target-specific `windows`, and test support in crates/space-lenser-core/Cargo.toml
- [X] T003 Add shadcn `button-group` and `spinner` components while preserving existing `table` and `button` components in src-web/src/components/ui/button-group.tsx and src-web/src/components/ui/spinner.tsx
- [X] T004 Create fs-explorer frontend feature directory and placeholder files in src-web/src/features/fs-explorer/FsExplorer.tsx, src-web/src/features/fs-explorer/PathButtonGroup.tsx, src-web/src/features/fs-explorer/ExplorerTable.tsx, src-web/src/features/fs-explorer/cache.ts, and src-web/src/features/fs-explorer/types.ts
- [X] T005 Create core driver module placeholders and exports in crates/space-lenser-core/src/driver.rs, crates/space-lenser-core/src/size.rs, crates/space-lenser-core/src/rules.rs, crates/space-lenser-core/src/cache.rs, crates/space-lenser-core/src/jobs.rs, and crates/space-lenser-core/src/lib.rs

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Define core data contracts and correctness primitives that every user story depends on.

**Critical**: No user story work should begin until these contracts compile.

- [X] T006 Define `Volume`, `ScanConfig`, `PathNode`, `DirectoryListing`, `ReadIssue`, `SizeMeasurement`, and state enums in crates/space-lenser-core/src/driver.rs
- [X] T007 [P] Define stable `DriverError` and driver result aliases in crates/space-lenser-core/src/driver.rs
- [X] T008 [P] Define Tauri DTOs for volumes, listings, nodes, scan config, issues, progress, and command errors in src-tauri/src/dto.rs
- [X] T009 [P] Define TypeScript DTO and UI state types matching Tauri IPC contracts in src-web/src/features/fs-explorer/types.ts
- [X] T010 Implement platform size identity and allocated-size helpers for Unix and fallback platforms in crates/space-lenser-core/src/size.rs
- [X] T011 [P] Add Unix hard-link dedupe and allocated-size unit tests in crates/space-lenser-core/src/size.rs
- [X] T012 Implement scan config fingerprinting and default scan configuration in crates/space-lenser-core/src/rules.rs
- [X] T013 [P] Add scan config validation and fingerprint unit tests in crates/space-lenser-core/src/rules.rs
- [X] T014 Implement in-memory directory cache entry, generation, freshness, and stale-state primitives in crates/space-lenser-core/src/cache.rs
- [X] T015 [P] Add cache generation and stale-state unit tests in crates/space-lenser-core/src/cache.rs
- [X] T016 Wire shared driver state container into Tauri application state in src-tauri/src/state.rs

**Checkpoint**: Foundation compiles and user story implementation can begin.

---

## Phase 3: User Story 1 - Browse Volumes Into Directories (Priority: P1) MVP

**Goal**: Users see volumes, open a volume, navigate into directories by row click, and return by grouped path buttons without waiting for full scans.

**Independent Test**: Open the explorer, select a mounted volume, click into at least three nested folders, and return to a parent path without restarting or waiting for a full-volume scan.

### Tests for User Story 1

- [X] T017 [P] [US1] Add core driver tests for volume listing and immediate directory listing behavior in crates/space-lenser-core/src/driver.rs
- [ ] T018 [P] [US1] Add frontend cache reducer tests for path insert and path retrieval behavior in src-web/src/features/fs-explorer/cache.ts
- [X] T019 [US1] Document manual volume-to-directory navigation validation in specs/002-fs-explorer-driver/quickstart.md

### Implementation for User Story 1

- [X] T020 [US1] Implement `HdDriver::list_volumes` using existing sysinfo drive discovery data in crates/space-lenser-core/src/driver.rs and crates/space-lenser-core/src/drives.rs
- [X] T021 [US1] Implement immediate `HdDriver::open_path` and `HdDriver::get_directory` cache-or-placeholder behavior in crates/space-lenser-core/src/driver.rs
- [X] T022 [US1] Implement shallow directory entry discovery with issue capture and symlink skip metadata in crates/space-lenser-core/src/scan.rs
- [X] T023 [US1] Add `fs_list_volumes`, `fs_open_path`, and `fs_get_directory` Tauri commands in src-tauri/src/commands.rs
- [X] T024 [US1] Register fs-explorer Tauri commands in src-tauri/src/lib.rs
- [X] T025 [US1] Implement frontend Tauri wrappers for volume and directory commands in src-web/src/api.ts
- [X] T026 [US1] Implement frontend path cache with generation-aware writes and reads in src-web/src/features/fs-explorer/cache.ts
- [X] T027 [US1] Implement volumes and directory rows table using shadcn Table in src-web/src/features/fs-explorer/ExplorerTable.tsx
- [X] T028 [US1] Implement grouped breadcrumb navigation using shadcn ButtonGroup in src-web/src/features/fs-explorer/PathButtonGroup.tsx
- [X] T029 [US1] Compose volume loading, row navigation, parent navigation, and cached listings in src-web/src/features/fs-explorer/FsExplorer.tsx
- [X] T030 [US1] Mount `FsExplorer` while preserving the existing Permissions button in src-web/src/App.tsx
- [X] T031 [US1] Run MVP validation commands from quickstart in specs/002-fs-explorer-driver/quickstart.md

**Checkpoint**: User Story 1 is functional and testable as the MVP.

---

## Phase 4: User Story 2 - Scan By Size Rules (Priority: P1)

**Goal**: Users can apply scan behavior where folders above an expansion threshold schedule the next level repeatedly, folders below a visible threshold are hidden, and hidden entries follow the configured visibility rule.

**Independent Test**: Use a fixture tree with folders above and below small test thresholds and verify large folders expand deeper while small folders and hidden entries are omitted or reported according to config.

### Tests for User Story 2

- [X] T032 [P] [US2] Add unit tests for expansion threshold, minimum visible threshold, and hidden-entry rules in crates/space-lenser-core/src/rules.rs
- [X] T033 [P] [US2] Add fixture scan tests for repeated one-level expansion and visible-folder filtering in crates/space-lenser-core/src/scan.rs
- [X] T034 [US2] Document small-threshold fixture validation for size rules in specs/002-fs-explorer-driver/quickstart.md

### Implementation for User Story 2

- [X] T035 [US2] Implement rule evaluation for expand-above, min-visible, hidden visibility, symlink skip, and filesystem-boundary skip in crates/space-lenser-core/src/rules.rs
- [X] T036 [US2] Replace full-result recursion with bounded `jwalk` directory measurement jobs that apply scan rules in crates/space-lenser-core/src/scan.rs
- [X] T037 [US2] Aggregate exact measured visible directory sizes using `SizeMeasurement` and hard-link dedupe in crates/space-lenser-core/src/scan.rs
- [X] T038 [US2] Persist applied scan config, visible children, and skip metadata into cache entries in crates/space-lenser-core/src/cache.rs
- [X] T039 [US2] Accept `ScanConfigDto` in fs-explorer Tauri commands and map it to core config in src-tauri/src/commands.rs
- [X] T040 [US2] Apply default scan configuration without adding extra visible controls in src-web/src/features/fs-explorer/FsExplorer.tsx
- [X] T041 [US2] Render filtered rows and issue metadata without additional navigation controls in src-web/src/features/fs-explorer/ExplorerTable.tsx

**Checkpoint**: User Stories 1 and 2 both work with rule-driven measurement.

---

## Phase 5: User Story 3 - Lazy-Load Deep Levels (Priority: P2)

**Goal**: Users can request deeper analysis while the first configured levels become usable quickly and deeper levels are deferred until rule or navigation demand.

**Independent Test**: Request a ten-level scan with preload depth two on a deep fixture and verify the first two levels become available before lower-priority jobs finish.

### Tests for User Story 3

- [ ] T042 [P] [US3] Add core tests for preload depth, requested depth, and deferred child scheduling in crates/space-lenser-core/src/jobs.rs
- [ ] T043 [P] [US3] Add cache tests for returning measured directories without backend rediscovery in crates/space-lenser-core/src/cache.rs
- [X] T044 [US3] Document ten-level lazy-load fixture validation in specs/002-fs-explorer-driver/quickstart.md

### Implementation for User Story 3

- [X] T045 [US3] Implement scan job registry, job ids, request ids, cancellation tokens, and worker scheduling in crates/space-lenser-core/src/jobs.rs
- [X] T046 [US3] Implement preload-depth scheduling and deferred deeper-level scheduling in crates/space-lenser-core/src/driver.rs
- [X] T047 [US3] Return partial listings with `has_more_depth`, `loaded_depth`, and working child nodes in crates/space-lenser-core/src/driver.rs
- [X] T048 [US3] Add `fs_start_scan` and `fs_cancel_job` command shells backed by the job registry in src-tauri/src/commands.rs
- [X] T049 [US3] Update frontend cache to store partial listings, placeholder rows, and loaded-depth metadata in src-web/src/features/fs-explorer/cache.ts
- [X] T050 [US3] Render partial and placeholder rows without blocking table row navigation in src-web/src/features/fs-explorer/ExplorerTable.tsx

**Checkpoint**: Deep scans are lazy, cached, and navigable before all deeper jobs complete.

---

## Phase 6: User Story 4 - Track Accurate Progress And Working Items (Priority: P2)

**Goal**: Users see honest job progress and per-row working states while active jobs discover additional work.

**Independent Test**: Start a scan on a large fixture, observe queued/working row states, and verify progress reaches complete only after all scheduled work is terminal.

### Tests for User Story 4

- [X] T051 [P] [US4] Add progress state transition tests for discovered, scheduled, completed, skipped, failed, and canceled units in crates/space-lenser-core/src/jobs.rs
- [X] T052 [P] [US4] Add DTO serialization tests for `FsProgressEvent` and `ProgressSnapshot` in src-tauri/src/dto.rs
- [X] T053 [US4] Document progress and row spinner validation in specs/002-fs-explorer-driver/quickstart.md

### Implementation for User Story 4

- [X] T054 [US4] Implement `ProgressSnapshot` accounting and terminal completion rules in crates/space-lenser-core/src/jobs.rs
- [X] T055 [US4] Emit ordered driver events for job queued, started, row updated, directory ready, progress snapshot, finished, and failed in crates/space-lenser-core/src/driver.rs
- [X] T056 [US4] Stream `FsProgressEvent` updates over Tauri v2 channels in src-tauri/src/commands.rs
- [X] T057 [US4] Consume Tauri channel progress events and reject stale generations in src-web/src/api.ts
- [X] T058 [US4] Update fs-explorer state from row and progress events in src-web/src/features/fs-explorer/FsExplorer.tsx
- [X] T059 [US4] Render shadcn Spinner for queued, working, stale, and refreshing rows without row-height reflow in src-web/src/features/fs-explorer/ExplorerTable.tsx

**Checkpoint**: Progress and row working states are accurate, streamed, and non-blocking.

---

## Phase 7: User Story 5 - Invalidate And Rediscover A Path (Priority: P3)

**Goal**: Users can refresh a specific path after filesystem changes without clearing unrelated cached directories.

**Independent Test**: Scan a fixture path, change one subtree, invalidate that path, and verify only that path and affected descendants refresh while unrelated cached paths remain available.

### Tests for User Story 5

- [X] T060 [P] [US5] Add core tests for path-only and descendant invalidation preserving unrelated cache entries in crates/space-lenser-core/src/cache.rs
- [X] T061 [P] [US5] Add job supersession tests for invalidated paths replacing older active jobs in crates/space-lenser-core/src/jobs.rs
- [X] T062 [US5] Document path invalidation validation in specs/002-fs-explorer-driver/quickstart.md

### Implementation for User Story 5

- [X] T063 [US5] Implement `HdDriver::invalidate_path` with stale marking, generation bumping, and affected job supersession in crates/space-lenser-core/src/driver.rs
- [X] T064 [US5] Implement cache descendant invalidation and unrelated-entry preservation in crates/space-lenser-core/src/cache.rs
- [X] T065 [US5] Add `fs_invalidate_path` Tauri command and invalidation DTO mapping in src-tauri/src/commands.rs
- [X] T066 [US5] Add frontend API wrapper for invalidating the current path without adding extra navigation controls in src-web/src/api.ts
- [ ] T067 [US5] Apply invalidation events and refreshed listings to frontend cache generations in src-web/src/features/fs-explorer/cache.ts
- [ ] T068 [US5] Wire path rediscovery state into FsExplorer without blocking breadcrumb or row navigation in src-web/src/features/fs-explorer/FsExplorer.tsx

**Checkpoint**: Path-specific refresh works without clearing unrelated cached results.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Final correctness, performance, platform, and documentation work across all user stories.

- [X] T069 [P] Add Windows allocated-size host validation notes for compressed and sparse files in specs/002-fs-explorer-driver/quickstart.md
- [X] T070 [P] Add macOS/Linux permission-denied and filesystem-boundary fixture notes in specs/002-fs-explorer-driver/quickstart.md
- [ ] T071 Review and bound large listing payload sizes before enabling any `rmp-serde` transfer path in src-tauri/src/dto.rs
- [X] T072 [P] Add frontend type coverage for fs-explorer cache and event handling in src-web/src/features/fs-explorer/types.ts
- [X] T073 Run Rust formatting and tests for the workspace using commands documented in specs/002-fs-explorer-driver/quickstart.md
- [X] T074 Run frontend typecheck and build using commands documented in specs/002-fs-explorer-driver/quickstart.md
- [ ] T075 Run desktop manual smoke validation for volumes, navigation, spinners, cache reuse, and invalidation in specs/002-fs-explorer-driver/quickstart.md

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup and blocks every user story.
- **User Story 1 (Phase 3)**: Depends on Foundation and is the MVP.
- **User Story 2 (Phase 4)**: Depends on Foundation and integrates with US1 surface.
- **User Story 3 (Phase 5)**: Depends on Foundation; practically easiest after US1 and US2 because it schedules rule-aware directory jobs.
- **User Story 4 (Phase 6)**: Depends on US3 job registry and event emission.
- **User Story 5 (Phase 7)**: Depends on US3 job registry and US4 generation/event handling.
- **Polish (Phase 8)**: Depends on all desired stories.

### User Story Dependencies

- **US1 Browse Volumes Into Directories**: MVP, no dependency on other user stories after Foundation.
- **US2 Scan By Size Rules**: Can be built after Foundation but should integrate with US1 directory rows for visible verification.
- **US3 Lazy-Load Deep Levels**: Uses Foundation cache and jobs; should follow US1 for navigation and US2 for rule-aware scheduling.
- **US4 Track Accurate Progress And Working Items**: Requires US3 job lifecycle to report real progress.
- **US5 Invalidate And Rediscover A Path**: Requires cache generations, job supersession, and progress events from US3/US4.

### Parallel Opportunities

- T003 and T004 can run alongside T001, T002, and T005 after no shared files overlap.
- T007, T008, T009, T011, T013, and T015 can be implemented in parallel after T006/T010/T012/T014 ownership is clear.
- Within US1, frontend table/breadcrumb/cache work can proceed in parallel with core volume/listing work once DTO shapes are stable.
- US2 rules tests and scan fixture tests can run in parallel before implementation.
- US3 cache tests and job scheduling tests can run in parallel.
- US4 progress tests and DTO serialization tests can run in parallel.
- US5 cache invalidation and job supersession tests can run in parallel.

---

## Parallel Example: User Story 1

```text
Task: "T017 [P] [US1] Add core driver tests for volume listing and immediate directory listing behavior in crates/space-lenser-core/src/driver.rs"
Task: "T018 [P] [US1] Add frontend cache reducer tests for path insert and path retrieval behavior in src-web/src/features/fs-explorer/cache.ts"

Task: "T027 [US1] Implement volumes and directory rows table using shadcn Table in src-web/src/features/fs-explorer/ExplorerTable.tsx"
Task: "T028 [US1] Implement grouped breadcrumb navigation using shadcn ButtonGroup in src-web/src/features/fs-explorer/PathButtonGroup.tsx"
```

## Parallel Example: User Story 2

```text
Task: "T032 [P] [US2] Add unit tests for expansion threshold, minimum visible threshold, and hidden-entry rules in crates/space-lenser-core/src/rules.rs"
Task: "T033 [P] [US2] Add fixture scan tests for repeated one-level expansion and visible-folder filtering in crates/space-lenser-core/src/scan.rs"
```

## Parallel Example: User Story 4

```text
Task: "T051 [P] [US4] Add progress state transition tests for discovered, scheduled, completed, skipped, failed, and canceled units in crates/space-lenser-core/src/jobs.rs"
Task: "T052 [P] [US4] Add DTO serialization tests for `FsProgressEvent` and `ProgressSnapshot` in src-tauri/src/dto.rs"
```

---

## Implementation Strategy

### MVP First

1. Complete Phase 1 and Phase 2.
2. Complete Phase 3 for User Story 1.
3. Stop and validate volume listing, directory row navigation, breadcrumb navigation, cache reuse, and the Permissions button.

### Incremental Delivery

1. Add US1 to restore a useful explorer surface.
2. Add US2 to make scan sizing and filtering product-relevant.
3. Add US3 to make deep exploration scalable.
4. Add US4 to make long-running work trustworthy.
5. Add US5 to correct stale paths without discarding all cache.

### Validation Gates

Run the Rust and frontend validation commands from [quickstart.md](./quickstart.md) after each completed story phase. Host-specific filesystem behavior that cannot be deterministic in CI should be recorded as manual validation in [quickstart.md](./quickstart.md).
