# Quickstart: Tauri Desktop Migration

## Prerequisites

- Rust toolchain with Rust 2024 support
- Node.js package manager for the frontend; use `pnpm` unless the implementation selects another
  package manager in tasks
- Tauri v2 development dependencies for the local platform

## Implementation Setup

1. Convert the repository to a Rust workspace.
2. Move deterministic scanner/domain logic into `crates/hd-analyzer-core`.
3. Add `src-tauri/` with Tauri v2 configuration, conservative main-window capability, backend
   state, and registered commands from `contracts/tauri-ipc.md`.
4. Add `src-web/` as a Vite + React + TypeScript frontend.
5. Configure Tailwind CSS, the `@/*` import alias, and shadcn/ui.
6. Add shadcn/ui components needed for the first pass: button, card, table, tabs, dialog, progress,
   scroll-area, badge, separator, tooltip, and alert.
7. Implement frontend views for drive selection, scan explorer, category distribution, and error
   log.

## Local Validation

Run Rust checks:

```bash
cargo fmt --check
cargo test
```

Run frontend checks from `src-web/`:

```bash
pnpm install
pnpm dlx shadcn@latest init
pnpm dlx shadcn@latest add button card table tabs dialog progress scroll-area badge separator tooltip alert
pnpm run typecheck
pnpm run build
```

Run desktop development smoke test:

```bash
cargo tauri dev
```

## Manual Desktop Validation

1. Launch the desktop app.
2. Confirm drives appear with label, filesystem, total, used, and free space.
3. Start a scan and confirm progress updates while the window remains responsive.
4. Open a directory, return to the parent, and return to drive selection.
5. Rescan a focused subtree and confirm totals update without losing navigation state.
6. Scan a path expected to produce permission errors and confirm unreadable paths appear in the
   error log.
7. Confirm hidden/unscanned space is separated from scanned directory totals at the scan root.
8. Confirm primary controls, tables, dialogs, progress states, badges, and tooltips use the shared
   shadcn/ui component system and have visible keyboard focus states.

## Build Check

After local functionality works, verify the Tauri build command:

```bash
cargo tauri build
```

Signing, notarization, app store packaging, and auto-update are out of scope for this feature.
