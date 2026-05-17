import { invoke } from "@tauri-apps/api/core"
import {
  checkFullDiskAccessPermission,
  requestFullDiskAccessPermission,
} from "tauri-plugin-macos-permissions-api"
import { Channel } from "@tauri-apps/api/core"
import { appLog } from "@/lib/logging"

import type {
  DirectoryListingDto,
  DriveDto as FsDriveDto,
  FsProgressEvent,
  InvalidationReceiptDto,
  ScanConfigDto,
  StartScanReceiptDto,
} from "./features/fs-explorer/types"

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
  return invokeLogged<DriveDto[]>("list_drives")
}

export async function checkPermissions(root?: string): Promise<PermissionCheckDto> {
  return invokeLogged<PermissionCheckDto>("check_permissions", { root })
}

export async function checkMacFilePermissions(): Promise<PermissionCheckDto> {
  await appLog.debug("macOS full disk access permission check started")
  try {
    const granted = await checkFullDiskAccessPermission()
    await appLog.info("macOS full disk access permission check completed", { granted })
    return {
      granted,
      message: granted ? "File permissions are enabled." : "File permissions are required before scanning.",
    }
  } catch (error) {
    await appLog.error("macOS full disk access permission check failed", { error })
    throw error
  }
}

export async function requestMacFilePermissions(): Promise<PermissionCheckDto> {
  await appLog.info("macOS full disk access permission request started")
  try {
    await requestFullDiskAccessPermission()
    await appLog.info("macOS full disk access permission request completed")
    return checkMacFilePermissions()
  } catch (error) {
    await appLog.error("macOS full disk access permission request failed", { error })
    throw error
  }
}

export async function startScan(root: string): Promise<ScanSessionDto> {
  return invokeLogged<ScanSessionDto>("start_scan", { root })
}

export async function getScanSession(sessionId: string): Promise<ScanSessionDto> {
  return invokeLogged<ScanSessionDto>("get_scan_session", { sessionId })
}

export async function listDirectoryEntries(sessionId: string, path: string): Promise<DirectoryEntryDto[]> {
  return invokeLogged<DirectoryEntryDto[]>("list_directory_entries", { sessionId, path })
}

export async function rescanSubtree(sessionId: string, path: string): Promise<ScanSessionDto> {
  return invokeLogged<ScanSessionDto>("rescan_subtree", { sessionId, path })
}

export async function getReadErrors(sessionId: string): Promise<ReadErrorDto[]> {
  return invokeLogged<ReadErrorDto[]>("get_read_errors", { sessionId })
}

export async function fsListVolumes(): Promise<FsDriveDto[]> {
  return invokeLogged<FsDriveDto[]>("fs_list_volumes")
}

export async function fsOpenPath(
  path: string,
  volumeRoot: string,
  config?: ScanConfigDto,
): Promise<DirectoryListingDto> {
  return invokeLogged<DirectoryListingDto>("fs_open_path", { path, volumeRoot, config })
}

export async function fsGetDirectory(
  path: string,
  volumeRoot: string,
  config?: ScanConfigDto,
): Promise<DirectoryListingDto> {
  return invokeLogged<DirectoryListingDto>("fs_get_directory", { path, volumeRoot, config })
}

export async function fsStartScan(
  path: string,
  volumeRoot: string,
  config: ScanConfigDto | undefined,
  onProgress: (event: FsProgressEvent) => void,
): Promise<StartScanReceiptDto> {
  const progressChannel = new Channel<FsProgressEvent>()
  progressChannel.onmessage = onProgress
  return invokeLogged<StartScanReceiptDto>("fs_start_scan", {
    path,
    volumeRoot,
    config,
    replaceExisting: true,
    progressChannel,
  })
}

export async function fsInvalidatePath(path: string, volumeRoot: string): Promise<InvalidationReceiptDto> {
  return invokeLogged<InvalidationReceiptDto>("fs_invalidate_path", {
    path,
    volumeRoot,
    scope: "path_and_descendants",
  })
}

async function invokeLogged<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  await appLog.debug(`ipc ${command} started`)
  try {
    const result = await invoke<T>(command, args)
    await appLog.debug(`ipc ${command} completed`)
    return result
  } catch (error) {
    await appLog.error(`ipc ${command} failed`, { error })
    throw error
  }
}
