# Spaceman Analysis Notes

**Source inspected**: `external/spaceman` cloned from `https://github.com/salihgerdan/spaceman`

## Useful Abstractions To Carry Forward

- **Scan handle**: Spaceman models a scan as a handle containing the root path, shared tree, completion flag, update flag, and termination flag. This maps well to a long-running job handle that the UI can observe independently from navigation.
- **Live tree updates**: The scanner updates a shared tree while traversal continues. The UI can read partial state and refresh as updates arrive instead of waiting for the scan to finish.
- **Chunked insertion**: Traversal results are inserted in small batches to reduce lock overhead while still producing frequent updates.
- **Tree node graph**: Each node stores id, parent, children, path, display name, depth, file/directory kind, and accumulated size. Parent size propagation is simple and useful for visible aggregate rows.
- **Filesystem boundary filtering**: On Unix, Spaceman compares each directory's device id with the root device and skips descendants for directories outside the original filesystem.
- **Mount filtering**: Volume discovery filters likely system boot partitions from Unix mount lists.

## Gaps This Feature Must Address

- **Progress is only approximate**: Spaceman estimates progress from root child count and completed root children. The fs-explorer needs progress that honestly tracks discovered work, completed work, skipped work, failures, and newly scheduled work.
- **Size semantics are incomplete**: Spaceman uses logical file length. The fs-explorer needs validated disk-usage sizing, including hard-link deduplication and platform-specific allocated-size behavior.
- **No lazy path cache contract**: Spaceman maintains a live tree for one scan but does not define per-path cache entries, stale state, or invalidation semantics.
- **No row-level job state contract**: Spaceman has a global update signal, but the fs-explorer needs per-path queued, working, complete, failed, skipped, stale, and partial states.
- **Limited navigation model**: Spaceman notes sub-directory navigation as planned. The fs-explorer requires volume-to-directory navigation as a primary behavior.
- **No scan-rule expansion model**: Spaceman performs traversal of a selected tree. The fs-explorer needs repeated size-threshold expansion and minimum-visible-folder filtering.

## Planning Implications

- Keep traversal, cached path state, and UI navigation as separate concerns.
- Treat progress as a state model, not only a percentage.
- Build size correctness before UI polish; incorrect totals will invalidate the explorer.
- Preserve live partial results, but avoid exposing a row as complete until its size semantics and child policy are final for the applied scan configuration.
