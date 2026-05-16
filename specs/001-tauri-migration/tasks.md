# Tasks: Tauri Desktop Migration

**Input**: Design documents from `/specs/001-tauri-migration/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Include focused Rust tests for scan rules, size calculations, category classification,
path compaction, navigation state, DTO mapping, and Tauri command behavior. Include frontend tests
or type/build checks for shadcn/ui surfaces where practical, plus manual desktop validation for
host-specific filesystem permissions and live Tauri behavior.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing
of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel with other marked tasks in the same phase when files do not overlap
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- **Rust workspace**: `Cargo.toml`, `crates/hd-analyzer-core/`, `src-tauri/`
- **Frontend**: `src-web/`
- **Contracts and validation docs**: `specs/001-tauri-migration/`

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Establish the workspace, Tauri shell, and React/shadcn frontend structure required by
every user story.

- [X] T001 Convert root package into a Cargo workspace with members for `crates/hd-analyzer-core` and `src-tauri` in Cargo.toml
- [X] T002 Create `crates/hd-analyzer-core/Cargo.toml` with shared Rust dependencies in crates/hd-analyzer-core/Cargo.toml
- [X] T003 Remove terminal app package metadata from the workspace
- [X] T004 Remove terminal app source files so the Tauri desktop app is the only supported product surface
- [X] T005 Create Tauri backend package files in `src-tauri/Cargo.toml`, `src-tauri/build.rs`, and `src-tauri/tauri.conf.json`
- [X] T006 Create conservative main-window capability file in `src-tauri/capabilities/main.json`
- [X] T007 Create Vite React TypeScript frontend scaffold in `src-web/package.json`, `src-web/index.html`, `src-web/tsconfig.json`, `src-web/tsconfig.app.json`, `src-web/vite.config.ts`, and `src-web/src/main.tsx`
- [X] T008 Configure Tailwind CSS and shadcn/ui base files in `src-web/src/styles.css`, `src-web/components.json`, and `src-web/src/lib/utils.ts`
- [X] T009 Add required shadcn/ui component files under `src-web/src/components/ui/` for button, card, table, tabs, dialog, progress, scroll-area, badge, separator, tooltip, and alert
- [X] T010 [P] Add frontend command scripts for `dev`, `build`, `typecheck`, and `lint` in `src-web/package.json`
- [X] T011 [P] Update project README setup notes for workspace, Tauri, frontend, and validation commands in README.md
- [X] T012 [P] Update quickstart command references if implementation scripts differ from the plan in specs/001-tauri-migration/quickstart.md

**Checkpoint**: Workspace structure exists, dependencies are declared, generated source paths match the implementation plan, and no terminal app wrapper is retained.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Extract reusable scanner/domain behavior and define the shared IPC/state boundary before
any story-specific UI or command work begins.

**CRITICAL**: No user story work can begin until this phase is complete.

- [X] T013 Create public core module layout in `crates/hd-analyzer-core/src/lib.rs`, `crates/hd-analyzer-core/src/drives.rs`, `crates/hd-analyzer-core/src/scan.rs`, `crates/hd-analyzer-core/src/categories.rs`, and `crates/hd-analyzer-core/src/paths.rs`
- [X] T014 Move Drive, FileKind, CategoryUsage, ReadError, ScanProgress, ScanResult, and formatting/path helpers from CLI code into `crates/hd-analyzer-core/src/lib.rs` and supporting modules
- [X] T015 Move drive discovery and macOS duplicate-volume handling into `crates/hd-analyzer-core/src/drives.rs`
- [X] T016 Move file kind detection, executable detection, and category collection into `crates/hd-analyzer-core/src/categories.rs`
- [X] T017 Move allocated disk usage, symlink skipping, filesystem-boundary checks, parallel traversal, and progress snapshots into `crates/hd-analyzer-core/src/scan.rs`
- [X] T018 Move compact path and ratio/byte formatting helpers into `crates/hd-analyzer-core/src/paths.rs`
- [X] T019 Add unit tests for category classification and executable fallback in `crates/hd-analyzer-core/src/categories.rs`
- [X] T020 Add unit tests for path compaction and ratio formatting in `crates/hd-analyzer-core/src/paths.rs`
- [X] T021 Add unit tests for Drive used-space saturating arithmetic and duplicate mount handling where platform-independent in `crates/hd-analyzer-core/src/drives.rs`
- [X] T022 Add scan regression test helpers for temporary directory scanning, symlink skipping, and directory totals in `crates/hd-analyzer-core/src/scan.rs`
- [X] T023 Remove deferred CLI wrapper task because terminal app support is no longer retained
- [X] T024 Create Tauri DTO types for DriveDto, ScanSessionDto, ScanProgressDto, ScanResultDto, DirectoryEntryDto, CategoryUsageDto, ReadErrorDto, and error codes in `src-tauri/src/dto.rs`
- [X] T025 Create Tauri managed application state for active sessions, scan workers, and result storage in `src-tauri/src/state.rs`
- [X] T026 Create Tauri command module skeleton with registered command names from the IPC contract in `src-tauri/src/commands.rs`
- [X] T027 Wire Tauri builder, managed state, command registration, and capabilities in `src-tauri/src/lib.rs`
- [X] T028 Add DTO mapping tests from core entities to IPC shapes in `src-tauri/src/dto.rs`
- [X] T029 Create frontend API wrapper for `invoke`, scan event subscription, and typed DTOs in `src-web/src/api.ts`
- [X] T030 Create frontend state model for drives, scan session, result navigation, read errors, and status states in `src-web/src/state.ts`
- [X] T031 Create root app shell and view routing placeholders in `src-web/src/App.tsx`
- [X] T032 Run `cargo fmt --check` and fix formatting issues in the Rust workspace
- [X] T033 Run `cargo test` and fix failing core/DTO tests before story work proceeds
- [X] T034 Run `npm install` and `npm run typecheck` from `src-web/` after frontend scaffold exists

**Checkpoint**: Core scan behavior is reusable and tested, Tauri command/state boundaries compile, and frontend state/API types exist.

---

## Phase 3: User Story 1 - Launch Desktop Analyzer (Priority: P1) MVP

**Goal**: A user can open HD Analyzer as a desktop application and see available drives without
starting a terminal UI.

**Independent Test**: Start the desktop app, confirm the drive list loads, and quit the app without
using terminal controls.

### Tests for User Story 1

- [ ] T035 [P] [US1] Add command test for successful drive listing and `NO_DRIVES`/`DRIVE_DISCOVERY_FAILED` mapping in `src-tauri/src/commands.rs`
- [ ] T036 [P] [US1] Add frontend type/build coverage for drive selection DTO rendering in `src-web/src/views/DriveSelection.tsx`
- [ ] T037 [US1] Document manual launch and drive-list validation steps in `specs/001-tauri-migration/quickstart.md`

### Implementation for User Story 1

- [X] T038 [US1] Implement `list_drives` Tauri command using `hd-analyzer-core` drive discovery in `src-tauri/src/commands.rs`
- [X] T039 [US1] Implement DriveSelection view with shadcn card/table rows, badges, alert empty/error state, and scan action in `src-web/src/views/DriveSelection.tsx`
- [X] T040 [US1] Integrate DriveSelection into the root app shell and initial loading/error states in `src-web/src/App.tsx`
- [X] T041 [US1] Add drive selection actions and selected-drive state transitions in `src-web/src/state.ts`
- [X] T042 [US1] Add Tauri window metadata, title, and dev/build frontend paths in `src-tauri/tauri.conf.json`
- [X] T043 [US1] Validate MVP with `npm run typecheck` from `src-web/` and `cargo test`
- [ ] T044 [US1] Run `cargo tauri dev` and manually confirm drive list, empty/error behavior, keyboard focus, and app quit flow

**Checkpoint**: User Story 1 is functional and independently demoable as the MVP.

---

## Phase 4: User Story 2 - Scan and Explore in Desktop UI (Priority: P2)

**Goal**: A user can start a scan from the desktop interface, watch progress while it runs, browse
directory results, return to parent paths, and rescan a focused subtree.

**Independent Test**: Select a drive, start a scan, verify progress updates, navigate into at least
one directory, return to its parent, and rescan a focused folder.

### Tests for User Story 2

- [ ] T045 [P] [US2] Add core tests for building sorted directory entries and virtual root rows in `crates/hd-analyzer-core/src/scan.rs`
- [ ] T046 [P] [US2] Add command tests for `start_scan`, stale session rejection, `list_directory_entries`, and `rescan_subtree` in `src-tauri/src/commands.rs`
- [ ] T047 [P] [US2] Add frontend type/build coverage for scan progress, explorer rows, and category usage state in `src-web/src/views/ScanExplorer.tsx`
- [ ] T048 [US2] Update manual scan/explorer/rescan validation steps in `specs/001-tauri-migration/quickstart.md`

### Implementation for User Story 2

- [ ] T049 [US2] Implement scan session creation, active-scan guard, worker launch, progress emission, and final result storage in `src-tauri/src/commands.rs`
- [X] T050 [US2] Implement `list_directory_entries` with root-boundary validation and sorted directory rows in `src-tauri/src/commands.rs`
- [ ] T051 [US2] Implement `rescan_subtree` with partial-result merge and focused-path preservation in `src-tauri/src/commands.rs`
- [X] T052 [US2] Add optional cancellation support or explicitly omit `cancel_scan` registration per contract in `src-tauri/src/commands.rs`
- [ ] T053 [US2] Implement scan progress subscription and stale-session filtering in `src-web/src/api.ts`
- [ ] T054 [US2] Implement scan session reducer/actions for scanning, complete, failed, and partial-rescan states in `src-web/src/state.ts`
- [ ] T055 [US2] Implement ScanExplorer view with shadcn table, progress, cards, tabs, badges, tooltips, and navigation/rescan controls in `src-web/src/views/ScanExplorer.tsx`
- [ ] T056 [US2] Implement category distribution panel with stable layout and category shares in `src-web/src/views/ScanExplorer.tsx`
- [X] T057 [US2] Integrate drive-to-scan and scan-to-explorer transitions in `src-web/src/App.tsx`
- [ ] T058 [US2] Validate with `cargo test`, `npm run typecheck`, and `npm run build`
- [ ] T059 [US2] Run `cargo tauri dev` and manually confirm progress responsiveness, directory navigation, parent navigation, and focused subtree rescan

**Checkpoint**: User Story 2 works independently after US1 and preserves responsive scan behavior.

---

## Phase 5: User Story 3 - Explain Permission Gaps (Priority: P3)

**Goal**: A user can see unreadable paths, skipped-content policy, and hidden/unscanned space so
permission gaps are not confused with scanned content.

**Independent Test**: Run a scan against a location with unreadable paths and verify the desktop UI
shows hidden/unscanned space plus a readable error log.

### Tests for User Story 3

- [ ] T060 [P] [US3] Add core tests for read-error preservation and hidden/unscanned byte derivation in `crates/hd-analyzer-core/src/scan.rs`
- [ ] T061 [P] [US3] Add command tests for `get_read_errors` and virtual hidden/unscanned entry behavior in `src-tauri/src/commands.rs`
- [ ] T062 [P] [US3] Add frontend type/build coverage for error log and permission alert rendering in `src-web/src/views/ErrorLog.tsx`
- [ ] T063 [US3] Document host-specific manual permission test setup in `specs/001-tauri-migration/quickstart.md`

### Implementation for User Story 3

- [ ] T064 [US3] Ensure scan results expose `hiddenUnscannedBytes` without mixing it into scanned totals in `crates/hd-analyzer-core/src/scan.rs`
- [ ] T065 [US3] Implement `get_read_errors` command and read-error DTO mapping in `src-tauri/src/commands.rs`
- [ ] T066 [US3] Add virtual hidden/unscanned row handling to directory entry responses in `src-tauri/src/commands.rs`
- [ ] T067 [US3] Implement ErrorLog view with shadcn alert, scroll-area, badge, and readable long-path handling in `src-web/src/views/ErrorLog.tsx`
- [ ] T068 [US3] Add UI transitions between hidden/unscanned explorer row and ErrorLog view in `src-web/src/App.tsx`
- [ ] T069 [US3] Add permission/error state actions to frontend state in `src-web/src/state.ts`
- [ ] T070 [US3] Validate with `cargo test`, `pnpm run typecheck`, and host-specific manual permission scan from quickstart

**Checkpoint**: User Story 3 preserves permission-aware transparency in the desktop UI.

---

## Phase 6: User Story 4 - Use Consistent Desktop Components (Priority: P3)

**Goal**: A user experiences the desktop analyzer through one consistent shadcn/ui component system
with predictable controls, dense results, dialogs, and error states.

**Independent Test**: Inspect the drive selection, scan explorer, category distribution, rescan
controls, and error log screens and verify they use the same component language, spacing, focus
states, and accessible control behavior.

### Tests for User Story 4

- [ ] T071 [P] [US4] Add component-surface checklist coverage for required shadcn/ui components in `specs/001-tauri-migration/contracts/frontend-ui.md`
- [ ] T072 [P] [US4] Add frontend lint/type coverage for shadcn imports and shared component usage in `src-web/src/components/ui/`
- [ ] T073 [US4] Add manual UI consistency and keyboard-focus validation steps in `specs/001-tauri-migration/quickstart.md`

### Implementation for User Story 4

- [ ] T074 [US4] Create shared desktop layout primitives for page shell, section headers, metric rows, and action bars in `src-web/src/components/AppLayout.tsx`
- [ ] T075 [US4] Replace any one-off buttons, panels, alerts, progress indicators, and tables with shadcn/ui components in `src-web/src/views/DriveSelection.tsx`
- [ ] T076 [US4] Replace any one-off buttons, panels, alerts, progress indicators, and tables with shadcn/ui components in `src-web/src/views/ScanExplorer.tsx`
- [ ] T077 [US4] Replace any one-off buttons, panels, alerts, progress indicators, and tables with shadcn/ui components in `src-web/src/views/ErrorLog.tsx`
- [ ] T078 [US4] Add consistent focus styles, dense row sizing, truncation rules, and responsive window layout CSS in `src-web/src/styles.css`
- [ ] T079 [US4] Add lucide-react icons for scan, rescan, back, drive, alert, folder, and close actions in `src-web/src/views/DriveSelection.tsx`, `src-web/src/views/ScanExplorer.tsx`, and `src-web/src/views/ErrorLog.tsx`
- [ ] T080 [US4] Validate UI contract manually against `specs/001-tauri-migration/contracts/frontend-ui.md` in a running `cargo tauri dev` session

**Checkpoint**: User Story 4 satisfies the shadcn/ui component-system contract across the desktop app.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final validation, documentation, cleanup, and performance checks across all user
stories.

- [ ] T081 [P] Update README usage, build, and development instructions for the Tauri app in README.md
- [ ] T082 [P] Update AGENTS.md if implementation plan paths or commands changed from specs/001-tauri-migration/plan.md
- [ ] T083 [P] Add regression tests for public core APIs that remained uncovered in `crates/hd-analyzer-core/src/lib.rs`
- [ ] T084 Review Tauri capability scope and remove unused permissions in `src-tauri/capabilities/main.json`
- [ ] T085 Review IPC DTOs against `specs/001-tauri-migration/contracts/tauri-ipc.md` and fix contract drift in `src-tauri/src/dto.rs`
- [ ] T086 Review frontend component coverage against `specs/001-tauri-migration/contracts/frontend-ui.md` and fix component drift in `src-web/src/`
- [ ] T087 Run `cargo fmt --check` from repository root
- [ ] T088 Run `cargo test` from repository root
- [ ] T089 Run `pnpm run typecheck` and `pnpm run build` from `src-web/`
- [ ] T090 Run `cargo tauri build` from repository root
- [ ] T091 Execute full quickstart validation and record any host-specific caveats in `specs/001-tauri-migration/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup completion and blocks all user stories.
- **User Story 1 (Phase 3)**: Depends on Foundational completion; defines the MVP.
- **User Story 2 (Phase 4)**: Depends on US1 drive selection and Foundational scan core.
- **User Story 3 (Phase 5)**: Depends on US2 scan results and directory entry behavior.
- **User Story 4 (Phase 6)**: Can start after US1 components exist, but final validation depends on US1-US3 screens.
- **Polish (Phase 7)**: Depends on all selected user stories.

### User Story Dependencies

- **US1 Launch Desktop Analyzer**: Independent after Foundation.
- **US2 Scan and Explore**: Requires US1 drive selection to start scans.
- **US3 Permission Gaps**: Requires US2 scan results and explorer rows.
- **US4 Consistent Desktop Components**: Can be partially parallel with US2/US3, but final pass needs all screens.

### Within Each User Story

- Tests and validation docs come before implementation where deterministic tests are practical.
- Core/domain behavior before Tauri command mapping.
- Tauri command mapping before frontend API integration.
- Frontend state before screen integration.
- Screen implementation before manual Tauri validation.

### Parallel Opportunities

- Setup tasks T010-T012 can run in parallel after structural files exist.
- Foundational module extraction tasks T015-T018 can be split by file after T013-T014.
- Core tests T019-T022 can run in parallel by module.
- US1 command and frontend coverage T035-T036 can run in parallel.
- US2 core, command, and frontend tests T045-T047 can run in parallel.
- US3 core, command, and frontend tests T060-T062 can run in parallel.
- US4 contract, frontend lint/type coverage, and validation docs T071-T073 can run in parallel.
- Polish docs/tests T081-T083 can run in parallel before final validation commands.

---

## Parallel Example: User Story 2

```bash
# Independent test/design tasks:
Task: "T045 [P] [US2] Add core tests for building sorted directory entries and virtual root rows in crates/hd-analyzer-core/src/scan.rs"
Task: "T046 [P] [US2] Add command tests for start_scan, stale session rejection, list_directory_entries, and rescan_subtree in src-tauri/src/commands.rs"
Task: "T047 [P] [US2] Add frontend type/build coverage for scan progress, explorer rows, and category usage state in src-web/src/views/ScanExplorer.tsx"
```

## Parallel Example: User Story 4

```bash
# Independent component-system tasks:
Task: "T071 [P] [US4] Add component-surface checklist coverage for required shadcn/ui components in specs/001-tauri-migration/contracts/frontend-ui.md"
Task: "T072 [P] [US4] Add frontend lint/type coverage for shadcn imports and shared component usage in src-web/src/components/ui/"
Task: "T073 [US4] Add manual UI consistency and keyboard-focus validation steps in specs/001-tauri-migration/quickstart.md"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 setup.
2. Complete Phase 2 foundation.
3. Complete Phase 3 launch/drive selection.
4. Stop and validate that the desktop app launches, lists drives, handles empty/error states, and exits cleanly.

### Incremental Delivery

1. US1: Launch desktop analyzer and list drives.
2. US2: Start scans, show progress, browse directories, rescan subtrees.
3. US3: Expose permission errors and hidden/unscanned space.
4. US4: Finalize shared shadcn/ui consistency across every screen.

### Validation Gates

1. After Phase 2: `cargo fmt --check`, `cargo test`, and `npm run typecheck`.
2. After each user story: story-specific manual validation from quickstart.
3. Before completion: `cargo test`, `npm run build`, `npm run tauri:dev` smoke test, and `npm run tauri:build`.

## Notes

- [P] tasks are safe to run in parallel only when their listed files do not overlap with another active task.
- Story labels map to user stories in `specs/001-tauri-migration/spec.md`.
- Do not expose broad frontend filesystem permissions; Rust commands own scanning behavior.
- If `cancel_scan` is deferred, omit it from Tauri registration and hide cancel UI per `contracts/tauri-ipc.md`.
- Terminal app support is removed; the Tauri desktop app is the supported product surface.
