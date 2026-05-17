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
- Start a scan on a fixture path and verify row states move from queued/partial to complete without blocking breadcrumb or row clicks.
- Use a deep fixture with requested depth `10` and preload depth `2`; the first loaded levels should be navigable while deeper rows continue to resolve.
- Use small local thresholds such as `1 MB` expand-above and `100 KB` min-visible to validate large-folder expansion and small-folder filtering without creating large files.

## Fixture Guidance

Use small thresholds in local fixtures to avoid creating huge files. For example, set expansion to `1 MB` and minimum visible folder size to `100 KB` while validating rule behavior. Keep the 1 GB and 100 MB values as product defaults or examples, not mandatory test data sizes.

On Unix hosts, create hard-link fixtures with `ln` and verify the two linked paths count allocated bytes once for the same scan. For permission tests, create an unreadable directory where the host allows permission changes and verify it appears as scan metadata instead of disappearing. For filesystem-boundary validation on macOS or Linux, use an attached volume or mounted image and verify boundary-skipped descendants are reported as metadata.

On Windows hosts, validate compressed and sparse files separately from normal files. Mark a test file compressed through file properties or `compact.exe`, create a sparse file with `fsutil sparse`, and verify the reported allocated size follows on-disk usage rather than logical length where the platform API permits it.
