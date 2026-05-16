# Feature Specification: Tauri Desktop Migration

**Feature Branch**: `001-tauri-migration`

**Created**: 2026-05-16

**Status**: Draft

**Input**: User description: "we need to plan to migrate current app into use tauri instead of a cli backend; we need to use shadcn for the front-end tauri app"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Launch Desktop Analyzer (Priority: P1)

As a user, I can open HD Analyzer as a desktop application and see my available drives without
starting a terminal UI.

**Why this priority**: This establishes the replacement product surface and proves that the
existing analyzer can move out of the CLI/TUI entry point.

**Independent Test**: Start the desktop app, confirm the drive list loads, and quit the app without
using terminal controls.

**Acceptance Scenarios**:

1. **Given** the application is installed locally, **When** the user launches it, **Then** the user
   sees available drives with total, used, and free space.
2. **Given** no drives can be detected, **When** the user launches the app, **Then** the app shows a
   clear error state and remains usable.

---

### User Story 2 - Scan and Explore in Desktop UI (Priority: P2)

As a user, I can start a scan from the desktop interface, watch progress while it runs, and browse
directory results after or during scanning.

**Why this priority**: The core value of HD Analyzer is responsive disk analysis, not just drive
listing.

**Independent Test**: Select a drive, start a scan, verify progress updates, navigate into at least
one directory, return to its parent, and rescan a focused folder.

**Acceptance Scenarios**:

1. **Given** a drive is selected, **When** the user starts scanning, **Then** progress updates without
   freezing the UI.
2. **Given** scan results are visible, **When** the user opens a directory, **Then** child directories
   are shown with size and share information.
3. **Given** a result entry is focused, **When** the user requests a rescan, **Then** that subtree is
   rescanned and the displayed totals update consistently.

---

### User Story 3 - Explain Permission Gaps (Priority: P3)

As a user, I can see which paths were unreadable or intentionally skipped so that hidden or
unscanned space is not confused with scanned content.

**Why this priority**: Desktop migration must preserve trust in the analyzer's numbers.

**Independent Test**: Run a scan against a location with unreadable paths and verify the desktop UI
shows hidden/unscanned space plus a readable error log.

**Acceptance Scenarios**:

1. **Given** a scan encounters unreadable directories, **When** results are displayed, **Then** the
   app exposes the unreadable paths and error messages.
2. **Given** drive used space exceeds scanned totals, **When** the user views root results, **Then**
   hidden/unscanned space is clearly separated from scanned directory totals.

---

### User Story 4 - Use Consistent Desktop Components (Priority: P3)

As a user, I experience the desktop analyzer through a consistent, polished component system that
makes controls, dense scan results, dialogs, and error states predictable.

**Why this priority**: Migrating to desktop should improve usability without forcing every screen
to invent its own visual and interaction patterns.

**Independent Test**: Inspect the drive selection, scan explorer, category distribution, rescan
controls, and error log screens and verify they use the same component language, spacing, focus
states, and accessible control behavior.

**Acceptance Scenarios**:

1. **Given** the user moves between desktop screens, **When** controls and result panels are shown,
   **Then** buttons, tables, cards, dialogs, and status indicators follow one shared design system.
2. **Given** a user navigates by keyboard, **When** focus moves through actions and result controls,
   **Then** focused elements are visible and activation behavior is consistent.

---

### Edge Cases

- What happens when a scan is already running and the user starts another scan?
- How does the app handle access-denied paths, skipped hidden entries, symlinks, and filesystem
  boundaries?
- How does the desktop layout behave in small windows and high-density result sets?
- How does the component system handle dense directory tables without wasting screen space?
- What happens when the app is closed while a scan thread is still running?
- How are platform-specific permission differences explained to the user?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST launch as a local desktop application using the requested Tauri delivery
  model.
- **FR-002**: System MUST preserve drive discovery with total, used, free, label, mount point, and
  filesystem information.
- **FR-003**: System MUST allow users to start, monitor, and complete scans from the desktop UI.
- **FR-004**: System MUST keep the UI responsive while scans run.
- **FR-005**: System MUST display directory size, share, category distribution, file count, directory
  count, and elapsed time derived from one consistent scan model.
- **FR-006**: System MUST support navigation into directories, returning to parents, returning to
  drive selection, and rescanning a focused subtree.
- **FR-007**: System MUST report unreadable paths, skipped content policy, and hidden/unscanned
  space honestly.
- **FR-008**: System MUST avoid symlink recursion and preserve filesystem-boundary behavior unless
  a future spec explicitly changes it.
- **FR-009**: System MUST expose clear loading, empty, error, scanning, complete, and partial-rescan
  states in the desktop UI.
- **FR-010**: System MUST provide an implementation path that separates reusable Rust scan logic
  from the desktop UI shell.
- **FR-011**: System MUST use shadcn/ui as the desktop frontend component system.
- **FR-012**: System MUST use accessible, keyboard-operable components for primary actions,
  navigation controls, dense tables, dialogs, and error states.
- **FR-013**: System MUST keep scan result views dense enough for repeated analysis work while
  preserving readable spacing, focus states, and responsive desktop window behavior.

### Key Entities *(include if feature involves data)*

- **Drive**: A selectable storage volume with label, mount point, total space, available space, used
  space, and filesystem.
- **ScanSession**: A running or completed scan with root path, progress, elapsed time, completion
  state, directory sizes, category totals, and read errors.
- **DirectoryEntry**: A browsable directory row with path, display name, size, share, and directory
  status.
- **ReadError**: A path and error message captured when the scanner cannot read filesystem content.
- **CategoryUsage**: File category and allocated-size total used for distribution display.
- **UiState**: Desktop navigation state including selected drive, current result path, selected
  entry, error-log position, and active scan status.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can launch the desktop app and start a drive scan without using a terminal.
- **SC-002**: During a long-running scan, progress remains visible and the UI accepts navigation or
  cancellation-related input within one second.
- **SC-003**: For a completed scan, total bytes, directory sizes, category totals, and displayed
  percentages remain internally consistent.
- **SC-004**: Permission-denied paths and hidden/unscanned space are visible from the results view.
- **SC-005**: Existing scanner behavior for symlink skipping, filesystem-boundary handling, hidden
  entry policy, and allocated-size calculation is preserved unless explicitly documented.
- **SC-006**: The migration plan identifies the reusable Rust core, desktop command boundary, and
  UI state transitions before implementation begins.
- **SC-007**: Core desktop screens use a shared component system so primary actions and result
  controls behave consistently across drive selection, scanning, exploration, and error review.

## Assumptions

- Tauri is a required delivery constraint from the user request.
- shadcn/ui is a required frontend component constraint from the user request.
- The first migration target is the current local desktop development platform; cross-platform
  packaging can be planned after the core desktop migration works.
- The current Rust scanner remains the source of truth for filesystem analysis.
- The CLI/TUI entry point can remain temporarily during migration if it lowers risk, but the target
  product surface is the Tauri desktop app.
- Elevated access guidance remains user-facing documentation rather than an automatic privilege
  escalation flow.
