# Implementation Plan: Filesystem Explorer Driver

**Branch**: `002-fs-explorer-driver` | **Date**: 2026-05-17 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/002-fs-explorer-driver/spec.md`

**Note**: This plan is produced by the `/speckit-plan` workflow and stops before task generation.

## Summary

Build the fs-explorer as the core navigation and analysis surface for Space Lenser. The work introduces a Rust `hd-driver` contract in `space-lenser-core` for volume listing, cached directory discovery, rule-driven scan jobs, invalidation, and progress snapshots. The Tauri v2 layer will expose this driver through commands plus channel-based streaming updates, while the React/shadcn frontend will render only the explorer surface: volumes, grouped breadcrumb navigation, clickable table rows, and per-row working indicators.

The scanner will replace the current full-result flow with asynchronous path jobs backed by `jwalk`, platform-specific allocated-size measurement, global hard-link deduplication, and honest progress accounting for discovered, scheduled, completed, skipped, failed, and canceled work.

## Technical Context

**Language/Version**: Rust 2024 core and Tauri backend; TypeScript 5.7, React 19, Vite 7 frontend.

**Primary Dependencies**: Existing workspace dependencies (`anyhow`, `humansize`, `rayon`, `serde`, `serde_json`, `sysinfo`, `tauri`) plus planned core dependencies:

- `jwalk` for parallel directory traversal with configurable depth, hidden-entry skipping, and read-dir hooks.
- `thiserror` for stable driver error types that map cleanly into Tauri responses.
- `nix` for POSIX filesystem/syscall details that are not exposed cleanly by `std`.
- `windows` as a target-specific dependency for `GetCompressedFileSizeW` and Windows file attribute handling.
- `rmp-serde` only behind the bulk-directory/snapshot transfer path if JSON IPC profiling shows payload serialization is the bottleneck. DTOs must remain format-neutral from the start.

**Storage**: In-memory per-path directory cache keyed by canonical path plus scan-affecting configuration fingerprint. No persistent database in v1.

**Testing**: `cargo fmt --check`, `cargo test`, focused Rust unit/integration tests for size semantics, hard-link dedupe, scan rule expansion, cache invalidation, progress state transitions, and Tauri DTO serialization. Frontend validation uses `npm run typecheck` and `npm run build` from `src-web`.

**Target Platform**: macOS first for local development, with Linux and Windows guarded by `cfg`. Unix uses allocated blocks and `(dev, ino)` identity. Windows uses on-disk size APIs for sparse/compressed files where available and falls back with explicit metadata when unsupported.

**Project Type**: Tauri v2 desktop app with a Rust workspace core and React/shadcn frontend.

**Performance Goals**:

- List volumes within 2 seconds on a typical desktop.
- Navigate at least three levels without waiting for a full-volume scan.
- Make the first two levels of a 10,000-entry fixture navigable within 5 seconds when configured as preload depth 2.
- Keep row updates under 500 ms after backend results become available for at least 95% of updates.
- Avoid UI interaction stalls over 100 ms while five scan jobs are active.

**Constraints**:

- The UI tree navigation must never block on scan completion.
- Symlinks are not followed by default.
- Filesystem boundary behavior is preserved unless an explicit config later changes it.
- Progress must not pretend the total tree size or total work count is known before discovery.
- Size semantics must report disk usage, not only logical length, when the platform exposes allocated size.
- Visible v1 scope is browsing, scanning, progress, caching, and invalidation only.

**Scale/Scope**: Single-user local or mounted filesystem exploration across large directory trees with concurrent jobs for different paths and one authoritative cache entry per path.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Fast, Responsive Disk Analysis**: PASS. Scan jobs run outside React rendering and Tauri command handling, progress streams incrementally, and navigation is served from cache, partial listings, or placeholders.
- **Accurate Filesystem Semantics**: PASS. The design requires Unix block-based allocated size, global hard-link dedupe, Windows allocated-size handling for compressed/sparse files, symlink skipping, and boundary metadata.
- **Terminal UX Is the Product**: JUSTIFIED VIOLATION. The active product direction is the Tauri desktop migration established in `specs/001-tauri-migration/plan.md`. This feature continues that migration and replaces terminal UI requirements with equivalent desktop responsiveness, visible state, and permission transparency.
- **Permission-Aware Transparency**: PASS. Access denied paths, hidden skips, unreadable entries, and boundary skips are first-class metadata on listings and progress snapshots.
- **Testable Rust Core**: PASS. Driver rules, sizing, cache state, progress accounting, and invalidation are planned as core Rust logic independent of the Tauri and React surfaces.

## Project Structure

### Documentation (this feature)

```text
specs/002-fs-explorer-driver/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── spaceman-analysis.md
├── contracts/
│   ├── frontend-ui.md
│   ├── hd-driver.md
│   └── tauri-ipc.md
└── tasks.md              # Created by /speckit-tasks, not by /speckit-plan
```

### Source Code (repository root)

```text
crates/space-lenser-core/src/
├── cache.rs              # Per-path directory cache and freshness state
├── driver.rs             # HdDriver/AnalysisProvider contract and orchestration
├── drives.rs             # Volume discovery using sysinfo
├── jobs.rs               # Async job registry, cancellation, replacement, progress
├── rules.rs              # Scan depth, preload, expansion, filtering rules
├── scan.rs               # jwalk traversal integration and directory measurement
├── size.rs               # Platform allocated-size and hard-link identity logic
└── lib.rs                # Public module exports

src-tauri/src/
├── commands.rs           # Tauri commands for fs-explorer operations
├── dto.rs                # Serializable request/response/event payloads
├── state.rs              # Shared driver state and channel/job registry
└── lib.rs                # Command registration

src-web/src/
├── api.ts                # Tauri invoke/channel wrappers
├── features/fs-explorer/
│   ├── FsExplorer.tsx    # Explorer composition
│   ├── PathButtonGroup.tsx
│   ├── ExplorerTable.tsx
│   ├── cache.ts
│   └── types.ts
├── components/ui/
│   ├── button-group.tsx  # Add from shadcn if missing
│   ├── spinner.tsx       # Add from shadcn if missing
│   └── table.tsx
└── App.tsx               # Mount fs-explorer and existing Permissions button
```

**Structure Decision**: Keep filesystem correctness and scan orchestration in `space-lenser-core`; keep Tauri as a thin transport/state boundary; keep React state and cache behavior in a feature-scoped `fs-explorer` directory using existing shadcn components.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Desktop UI instead of terminal UX | The project is already executing the Tauri migration and the user explicitly requested shadcn/Radix navigation and table components. | Returning to Ratatui would contradict the active migration plan and the requested product surface. |
| Driver, job registry, cache, and UI state as separate modules | Accurate lazy scans need independent ownership for traversal, progress, cache freshness, and transport. | A single scan function returning a full tree already exists and cannot support non-blocking navigation, invalidation, or row-level progress. |
| Platform-specific size modules | Correct disk-usage totals require Unix block/device/inode APIs and Windows allocated-size APIs. | `metadata().len()` is simpler but reports logical size and gives wrong totals for hard links, sparse files, and compressed files. |

## Phase 0 Research

Research decisions are documented in [research.md](./research.md). The main outcome is to use `jwalk` for parallel traversal, explicit platform size semantics for correctness, a path-keyed cache for lazy navigation, and Tauri v2 channels for progress streaming.

## Phase 1 Design

Design artifacts generated for task planning:

- [data-model.md](./data-model.md)
- [contracts/hd-driver.md](./contracts/hd-driver.md)
- [contracts/tauri-ipc.md](./contracts/tauri-ipc.md)
- [contracts/frontend-ui.md](./contracts/frontend-ui.md)
- [quickstart.md](./quickstart.md)

## Post-Design Constitution Check

- **Fast, Responsive Disk Analysis**: PASS. Contracts separate instant navigation reads from background measurement jobs and use row-level progress events.
- **Accurate Filesystem Semantics**: PASS. Data model includes `SizeIdentity`, platform measurement method, skip metadata, and exact completion rules.
- **Terminal UX Is the Product**: JUSTIFIED VIOLATION. The violation remains intentional and bounded to the established desktop migration.
- **Permission-Aware Transparency**: PASS. Contracts require skipped, denied, hidden, boundary, and unreadable counts and paths to survive through Tauri DTOs and UI state.
- **Testable Rust Core**: PASS. The driver API is transport-independent and can be tested with fixture filesystems before Tauri or React integration.
