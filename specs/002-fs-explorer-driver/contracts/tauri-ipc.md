# Contract: Tauri IPC

The Tauri layer is a transport boundary over `hd-driver`. Commands start work or read state. Channels stream progress and row updates.

## Commands

### `fs_list_volumes`

Request: none.

Response: `VolumeDto[]`.

### `fs_open_path`

Request:

```json
{
  "path": "/Users/example",
  "config": "ScanConfigDto",
  "progressChannel": "Channel<FsProgressEvent>"
}
```

Response: `DirectoryListingDto`.

Semantics: Return immediately with cached, partial, or placeholder data and use the channel for follow-up updates.

### `fs_start_scan`

Request:

```json
{
  "path": "/Users/example",
  "config": "ScanConfigDto",
  "replaceExisting": true,
  "progressChannel": "Channel<FsProgressEvent>"
}
```

Response:

```json
{ "jobId": "scan_123", "requestId": "req_456" }
```

### `fs_get_directory`

Request:

```json
{
  "path": "/Users/example",
  "configFingerprint": "sha256-or-stable-hash",
  "allowStale": true
}
```

Response: `DirectoryListingDto`.

### `fs_invalidate_path`

Request:

```json
{
  "path": "/Users/example/Downloads",
  "scope": "path_and_descendants"
}
```

Response:

```json
{
  "invalidatedPaths": ["/Users/example/Downloads"],
  "supersededJobIds": ["scan_123"]
}
```

### `fs_cancel_job`

Request:

```json
{ "jobId": "scan_123" }
```

Response:

```json
{ "jobId": "scan_123", "state": "canceled" }
```

## Channel Event: `FsProgressEvent`

Discriminated union fields: `event`, `data`.

Events:

- `job_queued`: `jobId`, `requestId`, `path`
- `job_started`: `jobId`, `requestId`, `path`
- `row_updated`: `jobId`, `requestId`, `path`, `row`
- `directory_ready`: `jobId`, `requestId`, `path`, `listing`
- `progress_snapshot`: `jobId`, `requestId`, `snapshot`
- `job_finished`: `jobId`, `requestId`, `path`, `summary`
- `job_failed`: `jobId`, `requestId`, `path`, `error`
- `path_invalidated`: `path`, `scope`, `generation`

## Payload Rules

- Paths are serialized as strings but validated as platform paths in Rust.
- Large listings must be paged or chunked before considering binary transport.
- Every event that mutates frontend cache must include enough identity to reject stale generations.
- Access-denied, hidden-skip, symlink-skip, and boundary-skip information must remain in DTOs.
