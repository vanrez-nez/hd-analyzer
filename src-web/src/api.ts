import { invoke } from "@tauri-apps/api/core"
import {
  checkFullDiskAccessPermission,
  requestFullDiskAccessPermission,
} from "tauri-plugin-macos-permissions-api"

export type DriveDto = {
  id: string
  label: string
  mountPoint: string
  totalSpace: number
  availableSpace: number
  usedSpace: number
  fileSystem: string
}

export type ScanSessionDto = {
  sessionId: string
  root: string
  status: "idle" | "scanning" | "complete" | "failed" | "partial_rescan" | "cancel_requested"
  errorMessage?: string | null
}

export type DirectoryEntryDto = {
  path: string
  displayName: string
  size: number
  share: number
  isDirectory: boolean
  isVirtual: boolean
  kind: "directory" | "hidden_unscanned"
}

export type ReadErrorDto = {
  path: string
  displayPath: string
  error: string
}

export type PermissionCheckDto = {
  granted: boolean
  message: string
}

export async function listDrives(): Promise<DriveDto[]> {
  return invoke<DriveDto[]>("list_drives")
}

export async function checkPermissions(root?: string): Promise<PermissionCheckDto> {
  return invoke<PermissionCheckDto>("check_permissions", { root })
}

export async function checkMacFilePermissions(): Promise<PermissionCheckDto> {
  const granted = await checkFullDiskAccessPermission()
  return {
    granted,
    message: granted ? "File permissions are enabled." : "File permissions are required before scanning.",
  }
}

export async function requestMacFilePermissions(): Promise<PermissionCheckDto> {
  await requestFullDiskAccessPermission()
  return checkMacFilePermissions()
}

export async function startScan(root: string): Promise<ScanSessionDto> {
  return invoke<ScanSessionDto>("start_scan", { root })
}

export async function getScanSession(sessionId: string): Promise<ScanSessionDto> {
  return invoke<ScanSessionDto>("get_scan_session", { sessionId })
}

export async function listDirectoryEntries(sessionId: string, path: string): Promise<DirectoryEntryDto[]> {
  return invoke<DirectoryEntryDto[]>("list_directory_entries", { sessionId, path })
}

export async function rescanSubtree(sessionId: string, path: string): Promise<ScanSessionDto> {
  return invoke<ScanSessionDto>("rescan_subtree", { sessionId, path })
}

export async function getReadErrors(sessionId: string): Promise<ReadErrorDto[]> {
  return invoke<ReadErrorDto[]>("get_read_errors", { sessionId })
}
