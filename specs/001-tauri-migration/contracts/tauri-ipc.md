# Contract: Tauri IPC

This contract defines the desktop frontend boundary for the Tauri migration. Command names use
snake_case on the Rust side and are invoked from TypeScript with matching names.

## Command: `list_drives`

**Purpose**: Load selectable drives for the desktop drive-selection view.

**Request**: none

**Response**:

```json
[
  {
    "id": "/System/Volumes/Data",
    "label": "Macintosh HD (/System/Volumes/Data)",
    "mountPoint": "/System/Volumes/Data",
    "totalSpace": 1000000000000,
    "availableSpace": 250000000000,
    "usedSpace": 750000000000,
    "fileSystem": "apfs"
  }
]
```

**Errors**:

- `NO_DRIVES`: no mounted drives were detected
- `DRIVE_DISCOVERY_FAILED`: backend could not query drive metadata

## Command: `start_scan`

**Purpose**: Start a full scan and stream progress to the frontend.

**Request**:

```json
{
  "root": "/System/Volumes/Data"
}
```

**Response**:

```json
{
  "sessionId": "scan-20260516-001",
  "root": "/System/Volumes/Data",
  "status": "scanning"
}
```

**Progress Stream**: emits `scan_progress` payloads until completion.

**Errors**:

- `INVALID_ROOT`: root path is empty or unavailable
- `SCAN_ALREADY_RUNNING`: another full scan is active
- `SCAN_START_FAILED`: worker could not be created

## Event/Channel: `scan_progress`

**Purpose**: Deliver incremental scan progress and optional snapshots.

**Payload**:

```json
{
  "sessionId": "scan-20260516-001",
  "root": "/System/Volumes/Data",
  "bytesScanned": 123456789,
  "filesScanned": 1200,
  "directoriesScanned": 95,
  "directorySizes": {
    "/System/Volumes/Data/Users": 98765432
  },
  "categories": [
    { "kind": "code", "label": "Code", "size": 12345, "share": 0.01 }
  ]
}
```

**Rules**:

- The backend SHOULD emit progress about every 250ms during active scans.
- The frontend MUST ignore progress with an unknown or stale `sessionId`.

## Event/Channel: `scan_finished`

**Purpose**: Deliver the authoritative final scan result.

**Payload**:

```json
{
  "sessionId": "scan-20260516-001",
  "status": "complete",
  "result": {
    "root": "/System/Volumes/Data",
    "bytesScanned": 1234567890,
    "filesScanned": 12000,
    "directoriesScanned": 950,
    "directorySizes": {
      "/System/Volumes/Data": 1234567890
    },
    "categories": [
      { "kind": "documents", "label": "Docs", "size": 1000000, "share": 0.08 }
    ],
    "readErrors": [
      {
        "path": "/System/Volumes/Data/private",
        "displayPath": "private",
        "error": "Permission denied"
      }
    ],
    "hiddenUnscannedBytes": 123456
  }
}
```

## Command: `list_directory_entries`

**Purpose**: Build the current explorer rows from a scan result and current path.

**Request**:

```json
{
  "sessionId": "scan-20260516-001",
  "path": "/System/Volumes/Data/Users"
}
```

**Response**:

```json
[
  {
    "path": "/System/Volumes/Data/Users/me",
    "displayName": "me",
    "size": 1000000,
    "share": 0.5,
    "isDirectory": true,
    "isVirtual": false,
    "kind": "directory"
  }
]
```

**Errors**:

- `UNKNOWN_SESSION`: no scan result exists for `sessionId`
- `PATH_OUTSIDE_ROOT`: requested path is not inside the scan root

## Command: `rescan_subtree`

**Purpose**: Rescan a focused directory and merge updated totals into the active result.

**Request**:

```json
{
  "sessionId": "scan-20260516-001",
  "path": "/System/Volumes/Data/Users/me/Downloads"
}
```

**Response**:

```json
{
  "sessionId": "scan-20260516-001",
  "path": "/System/Volumes/Data/Users/me/Downloads",
  "status": "partial_rescan"
}
```

**Progress/Finish**: uses the same `scan_progress` and `scan_finished` payload families with
`status: "partial_rescan"` until merged.

## Command: `get_read_errors`

**Purpose**: Load permission and read errors for the error-log view.

**Request**:

```json
{
  "sessionId": "scan-20260516-001"
}
```

**Response**:

```json
[
  {
    "path": "/System/Volumes/Data/private",
    "displayPath": "private",
    "error": "Permission denied"
  }
]
```

## Command: `cancel_scan`

**Purpose**: Request cancellation of an active full scan or partial rescan.

**Request**:

```json
{
  "sessionId": "scan-20260516-001"
}
```

**Response**:

```json
{
  "sessionId": "scan-20260516-001",
  "status": "cancel_requested"
}
```

**Rule**: If cancellation is not implemented in the first task set, this command MUST be omitted
from registration and the UI MUST not expose cancel controls.
