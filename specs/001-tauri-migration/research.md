# Research: Tauri Desktop Migration

## Decision: Use Tauri v2 manual setup inside the existing repository

**Rationale**: The current app already has Rust scanner logic and repository structure. Tauri's
manual setup supports adding Tauri to an existing project and creates a `src-tauri` directory with
configuration files. Official docs also show `cargo tauri dev` as a valid development path.

**Alternatives considered**:

- Generate a separate `create-tauri-app` project: rejected because it would split the existing Rust
  scanner from the app and require a larger later merge.
- Rewrite as a web-only app: rejected because local filesystem scanning requires native Rust access.

Source: https://v2.tauri.app/start/create-project/

## Decision: Use vanilla TypeScript + Vite for the first desktop frontend

**Rationale**: Tauri supports many frontend frameworks and also recommends vanilla TypeScript as a
simple starting point. HD Analyzer currently has no web frontend, so avoiding React/Svelte/Vue keeps
the first migration focused on scanner extraction, IPC, and desktop state.

**Alternatives considered**:

- React/Svelte/Vue: good future options, but unnecessary for the first migration surface.
- Rust web frontend with Leptos/Yew: keeps a Rust-only stack but adds WASM/build complexity before
  the desktop migration is proven.

Source: https://v2.tauri.app/start/create-project/

## Decision: Extract scanner/domain logic into `crates/hd-analyzer-core`

**Rationale**: Existing `src/app.rs` mixes drive discovery, scan threading, progress aggregation,
navigation state, category classification, and TUI-facing state. Tauri commands need reusable,
serializable scan operations that can be tested without a webview or terminal.

**Alternatives considered**:

- Move current `src/app.rs` directly under `src-tauri`: rejected because it preserves UI coupling and
  makes command tests harder.
- Remove the CLI immediately: rejected because a temporary CLI wrapper lowers migration risk and
  provides a comparison point for scanner parity.

## Decision: Use Tauri commands for request/response operations

**Rationale**: Tauri commands provide a typed Rust function boundary callable from the frontend, can
accept serializable arguments, return serializable data, return errors, and run asynchronously. This
fits drive listing, directory navigation, loading errors, and focused rescan commands.

**Alternatives considered**:

- Custom localhost HTTP server: rejected because it adds a network surface and duplicates Tauri IPC.
- Frontend filesystem plugin access: rejected because the Rust scanner already owns traversal rules
  and permission/error semantics.

Source: https://v2.tauri.app/develop/calling-rust/

## Decision: Stream scan progress with Tauri channels or events

**Rationale**: Scan progress is long-running and incremental. Tauri documentation identifies
channels as the recommended mechanism for streaming data to the frontend, and events are available
when a more dynamic frontend notification path is needed.

**Alternatives considered**:

- Polling `get_scan_status`: simpler, but wastes work and makes latency dependent on polling
  frequency.
- Returning only the final scan result: rejected because it violates the responsiveness principle.

Source: https://v2.tauri.app/develop/calling-rust/

## Decision: Use conservative Tauri capabilities

**Rationale**: Tauri capabilities constrain which permissions are exposed to webview windows. The
desktop frontend does not need broad direct filesystem permissions because Rust commands perform
drive scanning. Capabilities should allow only core app/window/event behavior and registered
commands needed by the main window.

**Alternatives considered**:

- Grant broad frontend filesystem permissions: rejected because it increases security scope and
  risks duplicate traversal logic.
- Disable capability review during migration: rejected because the desktop app will expose native
  filesystem functionality.

Source: https://v2.tauri.app/security/capabilities/

## Decision: Defer installer/signing work until after functional migration

**Rationale**: Tauri provides build and bundle tooling for platform-specific installers, and most
platforms require signing. The current feature focuses on local desktop migration; release packaging
should be planned once the app shell, scanner core, and UI are stable.

**Alternatives considered**:

- Include signed macOS/Windows/Linux packages in this phase: rejected because it expands scope beyond
  the functional migration.
- Ignore packaging entirely: rejected because the quickstart should still verify `tauri build` once
  dependencies are available.

Source: https://v2.tauri.app/distribute/
