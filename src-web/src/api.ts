import { invoke } from "@tauri-apps/api/core"
import {
  checkFullDiskAccessPermission,
  requestFullDiskAccessPermission,
} from "tauri-plugin-macos-permissions-api"
import { Channel } from "@tauri-apps/api/core"
import { appLog } from "@/lib/logging"

import type {
  DirectoryListingDto,
  DeleteReceiptDto,
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

export async function openFolder(path: string): Promise<void> {
  const folderPath = path.trim()
  if (folderPath.length === 0) {
    return
  }

  await appLog.info("open folder started", { path: folderPath })
  try {
    await invokeLogged<void>("fs_open_folder", { path: folderPath })
    await appLog.info("open folder completed", { path: folderPath })
  } catch (error) {
    await appLog.error("open folder failed", { error, path: folderPath })
    throw error
  }
}

export async function openTerminal(path: string): Promise<void> {
  const terminalPath = path.trim()
  if (terminalPath.length === 0) {
    return
  }

  await appLog.info("open terminal started", { path: terminalPath })
  try {
    await invokeLogged<void>("fs_open_terminal", { path: terminalPath })
    await appLog.info("open terminal completed", { path: terminalPath })
  } catch (error) {
    await appLog.error("open terminal failed", { error, path: terminalPath })
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

export async function deleteItems(
  paths: string[],
  volumeRoot: string,
  moveToTrash: boolean,
): Promise<DeleteReceiptDto> {
  const deletePaths = paths.filter((path) => path.trim().length > 0)
  if (deletePaths.length === 0) {
    return { deletedPaths: [] }
  }

  await appLog.info("delete items started", { pathCount: deletePaths.length, moveToTrash })
  try {
    const receipt = await invokeLogged<DeleteReceiptDto>("fs_delete_items", {
      paths: deletePaths,
      volumeRoot,
      moveToTrash,
    })
    await appLog.info("delete items completed", {
      deletedPathCount: receipt.deletedPaths.length,
      moveToTrash,
    })
    return receipt
  } catch (error) {
    await appLog.error("delete items failed", { error, pathCount: deletePaths.length, moveToTrash })
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
