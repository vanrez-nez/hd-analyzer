import { useState } from "react"
import { ShieldCheck } from "lucide-react"
import { requestMacFilePermissions } from "./api"
import { Button } from "@/components/ui/button"

export function App() {
  const [permissionChecking, setPermissionChecking] = useState(false)

  const handleRequestPermissions = () => {
    setPermissionChecking(true)
    requestMacFilePermissions()
      .catch(() => undefined)
      .finally(() => setPermissionChecking(false))
  }

  return (
    <main className="flex min-h-screen items-center justify-center p-6">
      <Button variant="secondary" disabled={permissionChecking} onClick={handleRequestPermissions}>
        <ShieldCheck data-icon="inline-start" />
        Permissions
      </Button>
    </main>
  )
}
