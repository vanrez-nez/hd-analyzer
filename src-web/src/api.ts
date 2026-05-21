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
  FsOpenProgressEvent,
  FsProgressEvent,
  InvalidationReceiptDto,
  ScanConfigDto,
  StartScanReceiptDto,
} from "./features/fs-explorer/types"

export type PermissionCheckDto = {
  granted: boolean
  message: string
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

export async function fsOpenPathWithProgress(
  path: string,
  volumeRoot: string,
  config: ScanConfigDto | undefined,
  onProgress: (event: FsOpenProgressEvent) => void,
): Promise<DirectoryListingDto> {
  const progressChannel = new Channel<FsOpenProgressEvent>()
  progressChannel.onmessage = onProgress
  return invokeLogged<DirectoryListingDto>("fs_open_path_with_progress", {
    path,
    volumeRoot,
    config,
    progressChannel,
  })
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
  replaceExisting = false,
): Promise<StartScanReceiptDto> {
  const progressChannel = new Channel<FsProgressEvent>()
  progressChannel.onmessage = onProgress
  return invokeLogged<StartScanReceiptDto>("fs_start_scan", {
    path,
    volumeRoot,
    config,
    replaceExisting,
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

export async function openItemLocation(paths: string[]): Promise<void> {
  const revealPaths = paths.filter((path) => path.trim().length > 0)
  if (revealPaths.length === 0) {
    return
  }

  await appLog.info("open item location started", { pathCount: revealPaths.length, paths: revealPaths })
  try {
    await invokeLogged<void>("fs_reveal_items", { paths: revealPaths })
    await appLog.info("open item location completed", { pathCount: revealPaths.length })
  } catch (error) {
    await appLog.error("open item location failed", { error, pathCount: revealPaths.length, paths: revealPaths })
    throw error
  }
}

export async function previewItem(path: string): Promise<void> {
  const previewPath = path.trim()
  if (previewPath.length === 0) {
    return
  }

  await appLog.info("preview item started", { path: previewPath })
  try {
    await invokeLogged<void>("fs_preview_item", { path: previewPath })
    await appLog.info("preview item completed", { path: previewPath })
  } catch (error) {
    await appLog.error("preview item failed", { error, path: previewPath })
    throw error
  }
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
