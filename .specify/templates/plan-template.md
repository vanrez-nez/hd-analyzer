# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Date**: [DATE] | **Spec**: [link]

**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

[Extract from feature spec: primary requirement + technical approach from research]

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: Rust 2024

**Primary Dependencies**: Ratatui, Crossterm, Rayon, Sysinfo, Anyhow, Humansize

**Storage**: Local filesystem metadata only; no persistent application storage unless specified

**Testing**: `cargo fmt --check`, `cargo test`, plus documented manual TUI validation when needed

**Target Platform**: Terminal environments supported by Crossterm; platform-specific filesystem
behavior must be guarded with `cfg`

**Project Type**: Rust terminal application

**Performance Goals**: Keep the TUI responsive during long scans and expose incremental progress

**Constraints**: Avoid blocking input/rendering, skip symlink recursion, preserve filesystem-boundary
behavior unless explicitly changed, and report unreadable/unscanned space honestly

**Scale/Scope**: Single-user local drive analysis across large directory trees

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Fast, Responsive Disk Analysis**: Does the design keep scan work outside the render/input loop,
  emit incremental progress, and define performance expectations for large directory trees?
- **Accurate Filesystem Semantics**: Are allocated-size calculations, symlink handling,
  filesystem-boundary behavior, category totals, and percentages consistent and documented?
- **Terminal UX Is the Product**: Are keyboard flows, footer help, error messages, and common
  terminal sizes covered by the design?
- **Permission-Aware Transparency**: Does the feature preserve or explicitly update access-denied,
  hidden/skipped entry, and unscanned-space reporting?
- **Testable Rust Core**: Are focused Rust tests or documented host-specific manual tests planned
  for scan rules, size calculations, category classification, path/navigation state, and regressions?

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
src/
├── main.rs              # Terminal lifecycle, event loop, key dispatch
├── app.rs               # Application state, drive discovery, scanning, classification
└── ui.rs                # Ratatui rendering

tests/                   # Add integration tests when behavior cannot live in unit tests
```

**Structure Decision**: [Document the selected structure and reference the real
directories captured above]

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
