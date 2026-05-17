import { useState } from "react"
import { ShieldCheck } from "lucide-react"
import { requestMacFilePermissions } from "./api"
import { Button } from "@/components/ui/button"
import { FsExplorer } from "@/features/fs-explorer/FsExplorer"

export function App() {
  const [permissionChecking, setPermissionChecking] = useState(false)

  const handleRequestPermissions = () => {
    setPermissionChecking(true)
    requestMacFilePermissions()
      .catch(() => undefined)
      .finally(() => setPermissionChecking(false))
  }

  return (
    <main className="flex min-h-screen flex-col gap-4 p-6">
      <div className="flex justify-end">
        <Button variant="secondary" disabled={permissionChecking} onClick={handleRequestPermissions}>
          <ShieldCheck data-icon="inline-start" />
          Permissions
        </Button>
      </div>
      <FsExplorer />
    </main>
  )
}
