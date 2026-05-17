# Quickstart: Filesystem Explorer Driver

## Install Missing UI Components

From `src-web`, add requested shadcn components if they are not present:

```bash
npx shadcn@latest add button-group
npx shadcn@latest add table
npx shadcn@latest add spinner
```

## Rust Validation

From the repository root:

```bash
cargo fmt --check
cargo test
```

Core tests should cover:

- Unix hard-link dedupe with two paths pointing at the same `(dev, ino)`.
- Unix allocated size using block counts where supported.
- Windows allocated-size fallback and compressed/sparse fixture coverage on a Windows host.
- Size-rule expansion above threshold and filtering below threshold.
- Cache hit, stale invalidation, descendant invalidation, and generation replacement.
- Progress transitions when traversal discovers more work after a job starts.

## Frontend Validation

From `src-web`:

```bash
npm run typecheck
npm run build
npm run tauri:dev
```

Manual checks in the desktop app:

- Volumes appear within 2 seconds.
- Clicking a volume opens its root listing.
- Breadcrumb `ButtonGroup` segments navigate to parent paths.
- Clicking a directory row navigates immediately, even while rows show spinners.
- Returning to a previously loaded path uses cache.
- Invalidating a path refreshes that path without clearing unrelated cached paths.

## Fixture Guidance

Use small thresholds in local fixtures to avoid creating huge files. For example, set expansion to `1 MB` and minimum visible folder size to `100 KB` while validating rule behavior. Keep the 1 GB and 100 MB values as product defaults or examples, not mandatory test data sizes.

On Unix hosts, create hard-link fixtures with `ln` and verify the two linked paths count allocated bytes once for the same scan. For permission tests, create an unreadable directory where the host allows permission changes and verify it appears as scan metadata instead of disappearing.
