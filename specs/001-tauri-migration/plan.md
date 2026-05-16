# Implementation Plan: Tauri Desktop Migration

**Branch**: `001-tauri-migration` | **Date**: 2026-05-16 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-tauri-migration/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Migrate HD Analyzer from a terminal-first application into a Tauri v2 desktop application while
preserving the current Rust filesystem scanner as the source of truth. The plan extracts scan,
drive, category, path, and permission/error behavior into a reusable Rust core, adds a Tauri shell
with typed IPC commands and scan progress streaming, and builds a desktop UI for drive selection,
scan progress, result browsing, category distribution, subtree rescans, and permission-gap review.

## Technical Context

**Language/Version**: Rust 2024 for scanner/core/Tauri backend; TypeScript for the desktop frontend

**Primary Dependencies**: Existing Rust dependencies (`anyhow`, `humansize`, `rayon`, `sysinfo`);
add Tauri v2 (`tauri`, `tauri-build`, `tauri-cli`), Serde for IPC DTOs, and a lightweight Vite +
vanilla TypeScript frontend

**Storage**: Local filesystem metadata only; no persistent application storage in this phase

**Testing**: `cargo fmt --check`, `cargo test`, frontend typecheck/build, Tauri dev smoke test, and
manual desktop validation for permission-dependent scenarios

**Target Platform**: Local desktop first, with macOS as the initial verification platform and
Tauri-supported Windows/Linux behavior guarded by platform-specific filesystem fallbacks

**Project Type**: Tauri desktop application with reusable Rust scan core and webview frontend

**Performance Goals**: Keep desktop UI responsive during long scans; publish progress snapshots at
least every 250ms while scanning; keep command responses small by sending snapshots/results rather
than raw file lists

**Constraints**: Preserve symlink skipping, filesystem-boundary behavior, allocated-size
calculation, hidden/skipped entry policy, read-error reporting, and hidden/unscanned-space
separation; do not request broad filesystem plugin permissions when Rust commands can own scanning
directly

**Scale/Scope**: Single-user local drive analysis across large directory trees; migration does not
include signed installers, auto-update, mobile targets, or privileged helper installation

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Fast, Responsive Disk Analysis**: PASS. Scan work remains in Rust worker threads and progress is
  streamed to the desktop UI through a Tauri command/channel boundary.
- **Accurate Filesystem Semantics**: PASS. Existing allocated-size, symlink, filesystem-boundary,
  directory-total, and category behavior is preserved in the extracted core.
- **Terminal UX Is the Product**: JUSTIFIED VIOLATION. This feature intentionally changes the
  primary product surface from TUI to Tauri desktop because the user explicitly requested migration
  away from the CLI/TUI backend. Existing keyboard and navigation affordances will be mapped into
  desktop equivalents where useful.
- **Permission-Aware Transparency**: PASS. Read errors and hidden/unscanned space remain first-class
  result data exposed in the desktop UI.
- **Testable Rust Core**: PASS. The migration extracts deterministic scan logic into a testable core
  crate and defines Rust tests for classification, path handling, totals, and navigation state.

## Project Structure

### Documentation (this feature)

```text
specs/001-tauri-migration/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── tauri-ipc.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
Cargo.toml               # Workspace root after migration
crates/
├── hd-analyzer-core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs       # Public scanner/domain API
│       ├── scan.rs      # Drive scanning, progress, read errors
│       ├── drives.rs    # Drive discovery and platform filtering
│       ├── categories.rs
│       └── paths.rs
├── hd-analyzer-cli/     # Optional temporary compatibility wrapper during migration
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── app.rs
│       └── ui.rs
src-tauri/
├── Cargo.toml
├── build.rs
├── tauri.conf.json
├── capabilities/
│   └── main.json
└── src/
    ├── lib.rs           # Tauri builder, state, command registration
    ├── commands.rs      # IPC commands and stream setup
    └── dto.rs           # Serde DTOs shared with frontend contract
src-web/
├── package.json
├── index.html
├── tsconfig.json
├── vite.config.ts
└── src/
    ├── main.ts
    ├── api.ts           # Tauri invoke/channel wrapper
    ├── state.ts
    ├── styles.css
    └── views/
        ├── DriveSelection.ts
        ├── ScanExplorer.ts
        └── ErrorLog.ts
tests/
└── core_regression.rs   # Integration tests when unit tests are insufficient
```

**Structure Decision**: Use a Rust workspace so the scanner is reusable by both the Tauri backend
and any temporary CLI/TUI wrapper. Put Tauri-specific code in `src-tauri/` and frontend code in
`src-web/`, matching Tauri's manual setup model for an existing project.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Terminal UX Is the Product | The requested feature changes HD Analyzer from TUI-first to Tauri desktop-first. | Keeping the Ratatui UI as the primary surface would not satisfy "use tauri instead of a cli backend". |
| Workspace split into core, optional CLI, and Tauri app | Separates scanner behavior from desktop shell and keeps deterministic logic testable. | Directly embedding current `src/app.rs` into `src-tauri` would couple UI state, scanner state, and IPC, making progress streaming and regression tests harder. |

## Phase 0 Research Summary

See [research.md](./research.md). Key decisions:

- Use Tauri v2 with manual initialization in the existing repo.
- Use vanilla TypeScript + Vite for the first frontend to avoid adding framework complexity before
  the migration proves the Rust/IPC boundary.
- Use typed Tauri commands for request/response operations and a streaming channel or event path
  for scan progress.
- Use Tauri capabilities conservatively; avoid broad filesystem frontend permissions because Rust
  scanner commands own filesystem access.

## Phase 1 Design Summary

See [data-model.md](./data-model.md), [contracts/tauri-ipc.md](./contracts/tauri-ipc.md), and
[quickstart.md](./quickstart.md).

**Post-design Constitution Check**:

- **Fast, Responsive Disk Analysis**: PASS. `start_scan` streams `ScanProgressEvent` updates and
  scan work stays outside the webview thread.
- **Accurate Filesystem Semantics**: PASS. Domain entities preserve allocated bytes, read errors,
  category totals, symlink skipping, and filesystem-boundary behavior.
- **Terminal UX Is the Product**: JUSTIFIED VIOLATION remains. The desktop UI replaces the terminal
  surface by design; keyboard affordances are retained as secondary accessibility controls.
- **Permission-Aware Transparency**: PASS. `ReadError` and hidden/unscanned rows are part of the IPC
  contract and UI states.
- **Testable Rust Core**: PASS. Core extraction and DTO mapping are independently testable before
  desktop rendering is complete.
