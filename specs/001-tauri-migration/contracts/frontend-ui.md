# Contract: shadcn/ui Desktop Frontend

This contract defines the desktop component surface for the Tauri migration. It complements the
IPC contract by making the required shadcn/ui usage testable.

## Required Component Set

The first implementation pass MUST configure shadcn/ui and include these components:

- `button`
- `card`
- `table`
- `tabs`
- `dialog`
- `progress`
- `scroll-area`
- `badge`
- `separator`
- `tooltip`
- `alert`

Additional shadcn/ui components MAY be added when a screen needs them, but the implementation MUST
avoid custom one-off replacements for components already listed here.

## Screen: Drive Selection

**Required UI behavior**:

- Drive list appears in dense cards or table rows.
- Primary scan action uses `button`.
- Drive metadata uses consistent labels/badges for filesystem and space state.
- Empty/error drive discovery state uses `alert`.

**Validation**:

- Keyboard focus can reach each drive and the scan action.
- Selected drive state is visually distinct.

## Screen: Scan Explorer

**Required UI behavior**:

- Directory results use a `table` or table-equivalent shadcn composition with stable row height.
- Scan progress uses `progress` plus textual status.
- Category distribution and explorer panes use consistent card/section styling.
- Rescan and navigation actions use shared buttons and tooltips where labels are not obvious.

**Validation**:

- Long directory names truncate or wrap without overlapping size/share columns.
- Progress updates do not reflow the entire page.
- Keyboard focus remains visible while scan data updates.

## Screen: Permission/Error Review

**Required UI behavior**:

- Permission gaps use `alert` for summary severity.
- Read errors appear in a scrollable list or table using `scroll-area`.
- Hidden/unscanned space uses `badge` or equivalent status styling distinct from scanned totals.

**Validation**:

- Error messages remain readable with long paths.
- The user can return to scan results without losing current scan state.

## Dialogs and Status States

**Required UI behavior**:

- Confirmation or blocking messages use `dialog`.
- Loading, empty, error, scanning, complete, and partial-rescan states use shared components rather
  than one-off markup.

**Validation**:

- Dialogs are keyboard-operable.
- Status states are visually consistent across drive selection, explorer, and error review.
