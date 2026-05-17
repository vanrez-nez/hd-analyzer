# Contract: Frontend UI

The fs-explorer UI is the primary product surface for this feature. It must remain minimal and avoid controls outside the requested navigation model.

## Surface

- The app keeps the existing Permissions button.
- The explorer starts at the volumes table.
- Selecting a volume opens its root path.
- Current path navigation uses shadcn `ButtonGroup` path segments with `ButtonGroupSeparator`.
- Directory and file contents use the shadcn `Table` component.
- Directory rows are clickable and navigate into that directory.
- Per-row working, queued, stale, and refreshing states use shadcn `Spinner` where applicable.

## Table Rows

Required visible fields:

- Name
- Kind
- Size
- State

Allowed metadata display:

- Access or skip issue count
- Hidden/boundary/symlink skip indicators when present

Rows must be stable in height while state changes. A spinner must not reflow the row.

## Navigation Rules

- Navigation never waits for a scan job to finish.
- If cached data exists, render it immediately.
- If only partial data exists, render partial rows and placeholders for working paths.
- If no data exists, render the requested path state and start discovery.
- Parent navigation uses breadcrumb buttons only.
- Child navigation uses table row clicks only.

## Cache Rules

- The frontend keeps a per-path cache mirroring backend generations.
- Newer backend generations replace older entries.
- Stale generations are ignored when an event arrives after invalidation or replacement.
- Returning to a cached path must not call the backend unless the cache is stale, missing, or requested with different scan-affecting config.

## Component Dependencies

Use these shadcn components:

- `button`
- `button-group`
- `table`
- `spinner`

Do not add unrelated navigation controls for v1.
