<!--
Sync Impact Report
Version change: template -> 1.0.0
Modified principles:
- Template principle 1 -> I. Fast, Responsive Disk Analysis
- Template principle 2 -> II. Accurate Filesystem Semantics
- Template principle 3 -> III. Terminal UX Is the Product
- Template principle 4 -> IV. Permission-Aware Transparency
- Template principle 5 -> V. Testable Rust Core
Added sections:
- Technology Constraints
- Development Workflow
Removed sections:
- None
Templates requiring updates:
- ✅ .specify/templates/plan-template.md
- ✅ .specify/templates/spec-template.md
- ✅ .specify/templates/tasks-template.md
- ✅ .specify/templates/checklist-template.md
- ⚠ .specify/templates/commands/*.md not present in this Spec Kit installation
- ✅ .specify/extensions/git/commands/*.md reviewed; no generic guidance updates required
- ✅ README.md reviewed; no principle references required
- ✅ AGENTS.md reviewed; existing Spec Kit pointer remains valid
Follow-up TODOs:
- None
-->
# HD Analyzer Constitution

## Core Principles

### I. Fast, Responsive Disk Analysis
HD Analyzer MUST keep the terminal interface responsive while scanning large directory trees.
Scanning work MUST run outside the input/render loop, emit incremental progress, and avoid
blocking user navigation. Changes that alter scanning behavior MUST define measurable
performance expectations for large drives and verify that progress remains observable during
long-running scans.

Rationale: The core value of the application is a real-time view into storage use; a stalled
terminal UI is a functional failure even when the final totals are correct.

### II. Accurate Filesystem Semantics
Filesystem measurements MUST use platform-appropriate disk usage semantics, not only logical
file length, when the platform exposes allocated size. The analyzer MUST avoid recursive traps
by skipping symlinks and MUST preserve filesystem-boundary behavior unless a feature explicitly
requires a documented change. Directory totals, category totals, and displayed percentages MUST
come from one consistent scan model.

Rationale: A disk analyzer is trusted only when its numbers explain actual storage pressure and
its traversal rules are predictable.

### III. Terminal UX Is the Product
Every user-facing change MUST preserve keyboard-first operation, clear visible state, and stable
layout in common terminal sizes. Screens MUST expose discoverable footer help, actionable error
messages, and navigation behavior that is consistent with the existing `j/k`, arrows, `h/l`,
Enter, Esc, and quit conventions unless a spec documents a better replacement.

Rationale: HD Analyzer is not a library with a thin UI; the TUI is the primary experience.

### IV. Permission-Aware Transparency
Access denied paths, unreadable directories, skipped hidden entries, and unscanned space MUST be
represented honestly. The application MUST NOT silently treat permission gaps as scanned space.
Features that change skip rules, hidden-file handling, or elevated-access guidance MUST update
the user-visible explanation and the error log behavior.

Rationale: Users run this tool specifically to find missing storage; hiding uncertainty makes the
results misleading.

### V. Testable Rust Core
New scan rules, size calculations, category classification, path compaction, navigation state, and
regression fixes MUST be covered by focused Rust tests or an explicitly documented manual test
when the behavior depends on host-specific filesystem permissions. Refactors MUST keep core
logic separable from terminal drawing where practical so behavior can be validated without a live
terminal session. The required local quality gate is `cargo fmt --check` and `cargo test`.

Rationale: Parallel filesystem code and terminal state machines are easy to regress; small tests
around deterministic logic are the cheapest protection.

## Technology Constraints

The project is a Rust 2024 terminal application. Production code MUST remain in Rust and SHOULD
prefer the existing stack: Ratatui for terminal rendering, Crossterm for terminal/input handling,
Rayon and standard threads/channels for concurrent scanning, Sysinfo for drive discovery, Anyhow
for fallible application boundaries, and Humansize for display formatting.

New dependencies MUST be justified in the implementation plan with their role, maintenance
surface, and why the standard library or existing dependency set is insufficient. Platform-specific
code MUST be guarded with `cfg` attributes and provide a safe fallback for unsupported platforms.

## Development Workflow

Specs MUST state the user-visible terminal behavior, filesystem traversal rules, performance
expectations, and permission/error handling affected by the feature. Plans MUST complete the
Constitution Check before research and after design. Tasks MUST be grouped by independently
testable user story and include formatting, tests, and any manual terminal validation needed for
the changed behavior.

Implementation MUST preserve unrelated user changes. Before delivery, contributors MUST run
`cargo fmt --check` and `cargo test` unless the environment prevents it; any skipped validation
MUST be reported with the reason.

## Governance

This constitution supersedes conflicting local conventions for HD Analyzer work. Amendments MUST
be made through `.specify/memory/constitution.md`, include a Sync Impact Report, and update any
templates or runtime guidance affected by changed principles.

Versioning follows semantic versioning. MAJOR versions remove or redefine principles in a
backward-incompatible way. MINOR versions add principles, sections, or materially expanded
governance. PATCH versions clarify language or fix non-semantic errors.

Every implementation plan and code review MUST verify compliance with the Core Principles. Any
intentional violation MUST be listed in the plan's Complexity Tracking table with the reason and
the simpler alternative that was rejected.

**Version**: 1.0.0 | **Ratified**: 2026-05-16 | **Last Amended**: 2026-05-16
