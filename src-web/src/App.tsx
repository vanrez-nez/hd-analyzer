import { useEffect, useState } from "react"
import {
  checkMacFilePermissions,
  getScanSession,
  listDirectoryEntries,
  listDrives,
  requestMacFilePermissions,
  startScan,
} from "./api"
import type { AppState } from "./state"
import { initialState } from "./state"
import { DriveSelection } from "./views/DriveSelection"
import { ScanExplorer } from "./views/ScanExplorer"

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === "object" && error !== null) {
    const maybeError = error as { message?: unknown; code?: unknown }
    if (typeof maybeError.message === "string") return maybeError.message
    if (typeof maybeError.code === "string") return maybeError.code
    try {
      return JSON.stringify(error)
    } catch {
      return "An unknown error occurred."
    }
  }
  return String(error)
}

export function App() {
  const [state, setState] = useState<AppState>({ ...initialState, loading: true })

  useEffect(() => {
    let cancelled = false

    listDrives()
      .then(async (drives) => {
        if (cancelled) return
        const selectedDrive = drives[0]
        if (!selectedDrive) {
          setState((current) => ({
            ...current,
            drives,
            selectedDriveId: undefined,
            permissionGranted: false,
            permissionChecking: false,
            permissionMessage: "No mounted drives were detected on this machine.",
            loading: false,
            error: undefined,
          }))
          return
        }

        const permission = await checkMacFilePermissions()
        if (cancelled) return
        setState((current) => ({
          ...current,
          drives,
          selectedDriveId: selectedDrive.id,
          permissionGranted: permission.granted,
          permissionChecking: false,
          permissionMessage: permission.granted ? undefined : permission.message,
          loading: false,
          error: undefined,
        }))
      })
      .catch((error: unknown) => {
        if (cancelled) return
        setState((current) => ({
          ...current,
          loading: false,
          permissionChecking: false,
          permissionGranted: false,
          error: errorMessage(error),
        }))
      })

    return () => {
      cancelled = true
    }
  }, [])

  const handleStartScan = () => {
    const selected = state.drives.find((drive) => drive.id === state.selectedDriveId)
    if (!selected || state.permissionGranted !== true) return

    setState((current) => ({ ...current, loading: true, error: undefined }))
    startScan(selected.mountPoint)
      .then((session) => {
        setState((current) => ({
          ...current,
          session,
          currentPath: session.root,
          entries: [],
          screen: "results",
          loading: true,
        }))
      })
      .catch((error: unknown) => {
        setState((current) => ({
          ...current,
          loading: false,
          error: errorMessage(error),
        }))
      })
  }

  const handleSelectDrive = (driveId: string) => {
    const selected = state.drives.find((drive) => drive.id === driveId)
    if (!selected) return

    setState((current) => ({
      ...current,
      selectedDriveId: driveId,
      permissionChecking: true,
      permissionGranted: undefined,
      permissionMessage: undefined,
      error: undefined,
    }))
    checkMacFilePermissions()
      .then((permission) => {
        setState((current) =>
          current.selectedDriveId === driveId
            ? {
                ...current,
                permissionGranted: permission.granted,
                permissionChecking: false,
                permissionMessage: permission.granted ? undefined : permission.message,
              }
            : current,
        )
      })
      .catch((error: unknown) => {
        setState((current) =>
          current.selectedDriveId === driveId
            ? {
                ...current,
                permissionChecking: false,
                permissionGranted: false,
                permissionMessage: undefined,
                error: errorMessage(error),
              }
            : current,
        )
      })
  }

  const handleRequestPermissions = () => {
    setState((current) => ({
      ...current,
      permissionChecking: true,
      error: undefined,
    }))
    requestMacFilePermissions()
      .then((permission) => {
        setState((current) => ({
          ...current,
          permissionGranted: permission.granted,
          permissionChecking: false,
          permissionMessage: permission.granted ? undefined : permission.message,
        }))
      })
      .catch((error: unknown) => {
        setState((current) => ({
          ...current,
          permissionChecking: false,
          permissionGranted: false,
          permissionMessage: undefined,
          error: errorMessage(error),
        }))
      })
  }

  useEffect(() => {
    if (state.screen !== "results" || !state.session || !state.currentPath) return
    if (state.session.status !== "scanning") return

    let cancelled = false
    const poll = () => {
      getScanSession(state.session!.sessionId)
        .then((session) => {
          if (cancelled) return

          if (session.status === "complete") {
            listDirectoryEntries(session.sessionId, session.root)
              .then((entries) => {
                if (cancelled) return
                setState((current) => ({
                  ...current,
                  session,
                  currentPath: session.root,
                  entries,
                  loading: false,
                  error: undefined,
                }))
              })
              .catch((error: unknown) => {
                if (cancelled) return
                setState((current) => ({
                  ...current,
                  session,
                  loading: false,
                  error: errorMessage(error),
                }))
              })
            return
          }

          if (session.status === "failed") {
            setState((current) => ({
              ...current,
              session,
              loading: false,
              error: session.errorMessage || "Scan failed.",
            }))
            return
          }

          setTimeout(poll, 750)
        })
        .catch((error: unknown) => {
          if (cancelled) return
          setState((current) => ({
            ...current,
            loading: false,
            error: errorMessage(error),
          }))
        })
    }

    poll()

    return () => {
      cancelled = true
    }
  }, [state.screen, state.session?.sessionId, state.session?.status, state.currentPath])

  const refreshEntries = () => {
    if (!state.session || !state.currentPath) return

    setState((current) => ({ ...current, loading: true }))
    listDirectoryEntries(state.session.sessionId, state.currentPath)
      .then((entries) => {
        setState((current) => ({ ...current, entries, loading: false, error: undefined }))
      })
      .catch((error: unknown) => {
        setState((current) => ({
          ...current,
          loading: false,
          error: errorMessage(error),
        }))
      })
  }

  if (state.screen === "results" && state.session && state.currentPath) {
    return (
      <main className="min-h-screen p-6">
        <ScanExplorer
          session={state.session}
          currentPath={state.currentPath}
          entries={state.entries}
          loading={state.loading}
          error={state.error}
          onBackToDrives={() => setState((current) => ({ ...current, screen: "drive_selection", loading: false }))}
          onRefreshEntries={refreshEntries}
        />
      </main>
    )
  }

  return (
    <main className="min-h-screen p-6">
      <DriveSelection
        drives={state.drives}
        selectedDriveId={state.selectedDriveId}
        loading={state.loading}
        error={state.error}
        permissionGranted={state.permissionGranted}
        permissionChecking={state.permissionChecking}
        permissionMessage={state.permissionMessage}
        onSelectDrive={handleSelectDrive}
        onRequestPermissions={handleRequestPermissions}
        onStartScan={handleStartScan}
      />
    </main>
  )
}
