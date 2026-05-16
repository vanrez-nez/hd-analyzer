import type {
  DirectoryEntryDto,
  DriveDto,
  ReadErrorDto,
  ScanSessionDto,
} from "./api"

export type Screen = "drive_selection" | "results" | "error_log"

export type AppState = {
  screen: Screen
  drives: DriveDto[]
  selectedDriveId?: string
  session?: ScanSessionDto
  currentPath?: string
  entries: DirectoryEntryDto[]
  readErrors: ReadErrorDto[]
  permissionGranted?: boolean
  permissionChecking: boolean
  permissionMessage?: string
  loading: boolean
  error?: string
}

export const initialState: AppState = {
  screen: "drive_selection",
  drives: [],
  entries: [],
  readErrors: [],
  permissionChecking: false,
  loading: false,
}

export function selectDrive(state: AppState, driveId: string): AppState {
  return {
    ...state,
    selectedDriveId: driveId,
    error: undefined,
  }
}
