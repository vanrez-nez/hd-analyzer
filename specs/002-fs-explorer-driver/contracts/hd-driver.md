# Contract: HD Driver Core API

The `hd-driver` is a Rust core abstraction. It must be usable from tests without Tauri and exposed through Tauri DTOs without leaking UI concerns into scan logic.

## Trait Shape

```rust
trait HdDriver {
    fn list_volumes(&self) -> Result<Vec<Volume>, DriverError>;
    fn open_path(&self, request: OpenPathRequest) -> Result<DirectoryListing, DriverError>;
    fn start_scan(&self, request: StartScanRequest, sink: ProgressSink) -> Result<ScanJobId, DriverError>;
    fn get_directory(&self, request: GetDirectoryRequest) -> Result<DirectoryListing, DriverError>;
    fn invalidate_path(&self, request: InvalidatePathRequest) -> Result<InvalidationReceipt, DriverError>;
    fn cancel_job(&self, job_id: ScanJobId) -> Result<CancelReceipt, DriverError>;
}
```

The concrete implementation may use internal channels, worker pools, and shared state, but callers observe only these contracts.

## Requests

### OpenPathRequest

Fields: `path`, `config`, `request_id`.

Semantics: Return the best currently available listing immediately. If no fresh listing exists, return a partial or placeholder listing and schedule discovery according to `config`.

### StartScanRequest

Fields: `path`, `config`, `replace_existing`, `request_id`.

Semantics: Start or reuse background jobs for the requested path. If `replace_existing` is true, older non-terminal jobs for the same path/config are superseded.

### GetDirectoryRequest

Fields: `path`, `config_fingerprint`, `allow_stale`.

Semantics: Read the cache without forcing a full rescan. If stale data is returned, `freshness` must be explicit.

### InvalidatePathRequest

Fields: `path`, `scope`, `request_id`.

Semantics: Mark the path and requested descendant scope stale, cancel or supersede affected jobs, and schedule rediscovery if the path is visible or explicitly requested.

## ProgressSink

`ProgressSink` receives ordered `DriverEvent` values for one request or job stream:

- `job_queued`
- `job_started`
- `row_updated`
- `directory_ready`
- `progress_snapshot`
- `job_finished`
- `job_failed`
- `path_invalidated`

Events must include `job_id`, `request_id`, `path`, and `generation` when they can affect cache state.

## Error Semantics

Stable error codes:

- `invalid_path`
- `path_outside_volume`
- `permission_denied`
- `job_not_found`
- `stale_request`
- `driver_unavailable`
- `size_unavailable`
- `volume_unmounted`
- `platform_unsupported`

Errors for individual paths should usually become `ReadIssue` metadata instead of failing the whole job.
