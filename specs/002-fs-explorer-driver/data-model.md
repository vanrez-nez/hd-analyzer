# Data Model: Filesystem Explorer Driver

## Volume

**Fields**: `id`, `display_name`, `root_path`, `mount_point`, `filesystem`, `total_bytes`, `available_bytes`, `is_removable`, `is_accessible`, `access_issue`.

**Relationships**: A volume is the root navigation target for directory listings and scan jobs.

**Validation**: `root_path` must be absolute. Capacity values are optional only when the platform cannot provide them.

## ScanConfig

**Fields**: `start_path`, `requested_depth`, `preload_depth`, `show_hidden`, `expand_above_bytes`, `min_visible_folder_bytes`, `stay_on_filesystem`, `follow_symlinks`.

**Validation**:

- `requested_depth >= 0`.
- `preload_depth <= requested_depth`.
- `follow_symlinks` defaults to `false`.
- `stay_on_filesystem` defaults to `true`.
- Thresholds must be non-negative.

## PathNode

**Fields**: `path`, `name`, `kind`, `parent_path`, `depth_from_request`, `size`, `state`, `visibility`, `children_known`, `active_job_id`, `issues`.

**Relationships**: A node belongs to one `DirectoryListing`; directory nodes can have their own cached listing.

**States**: `queued`, `working`, `partial`, `complete`, `skipped`, `failed`, `stale`, `canceled`.

## DirectoryListing

**Fields**: `path`, `config_fingerprint`, `children`, `total_visible_size`, `total_measured_size`, `state`, `loaded_depth`, `has_more_depth`, `issues`, `updated_at`.

**Relationships**: Stored in a `DirectoryCacheEntry`; produced by one or more scan jobs.

**Validation**: A listing is `complete` only when all visible children for the requested depth and rules have terminal states.

## DirectoryCacheEntry

**Fields**: `path`, `config_fingerprint`, `listing`, `freshness`, `active_jobs`, `generation`.

**Freshness states**: `fresh`, `partial`, `stale`, `invalidating`, `missing`.

**Validation**: A newer generation supersedes older job results for the same path and configuration.

## ScanJob

**Fields**: `job_id`, `request_id`, `root_path`, `config`, `scope`, `state`, `created_at`, `started_at`, `finished_at`, `cancel_token`, `progress`.

**States**: `queued`, `running`, `completed`, `failed`, `canceled`, `superseded`.

**Transitions**:

- `queued -> running`
- `running -> completed | failed | canceled | superseded`
- `queued -> canceled | superseded`

## ProgressSnapshot

**Fields**: `job_id`, `request_id`, `state`, `scheduled_units`, `discovered_units`, `completed_units`, `active_units`, `skipped_units`, `failed_units`, `canceled_units`, `bytes_measured`, `active_paths`, `issues`.

**Validation**: Progress is terminal only when `active_units == 0` and every scheduled unit has a terminal outcome. Percentages are derived presentation data, not authoritative state.

## SizeMeasurement

**Fields**: `allocated_bytes`, `logical_bytes`, `measurement_method`, `identity`, `deduped`.

**Measurement methods**: `unix_blocks`, `windows_compressed_size`, `logical_fallback`, `directory_aggregate`.

**Validation**: `allocated_bytes` must use the platform allocated-size method when available. Hard-linked file content already counted for the scan must contribute zero additional allocated bytes and set `deduped = true`.

## SizeIdentity

**Fields**: `platform`, `dev`, `ino`, `file_index`, `volume_serial`.

**Validation**: Unix identity uses `(dev, ino)` when available. Windows identity uses the best available stable file identity for dedupe, with unsupported cases flagged in `issues`.

## ReadIssue

**Fields**: `path`, `kind`, `message`, `job_id`, `when`.

**Kinds**: `permission_denied`, `not_found`, `not_directory`, `hidden_skipped`, `filesystem_boundary`, `symlink_skipped`, `metadata_failed`, `size_unavailable`, `volume_unmounted`.

## InvalidationRequest

**Fields**: `path`, `scope`, `reason`, `requested_at`.

**Scopes**: `path_only`, `path_and_descendants`.

**Validation**: Invalidation must not remove unrelated cache entries and must supersede older active jobs for the invalidated path scope.
