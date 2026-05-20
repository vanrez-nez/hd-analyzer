import { useState } from "react"
import { ShieldCheck } from "lucide-react"
import { requestMacFilePermissions } from "./api"
import { Button } from "@/components/ui/button"
import { Layout } from "./Layout"
import { useSystemColorScheme } from "@/lib/use-system-color-scheme"

export function App() {
  const [permissionChecking, setPermissionChecking] = useState(false)
  const colorScheme = useSystemColorScheme()

  const handleRequestPermissions = () => {
    setPermissionChecking(true)
    requestMacFilePermissions()
      .catch(() => undefined)
      .finally(() => setPermissionChecking(false))
  }

  return (
    <main className="flex h-screen min-h-0 flex-col gap-4 overflow-hidden p-6">
      <div className="flex justify-end">
        <Button variant="secondary" disabled={permissionChecking} onClick={handleRequestPermissions}>
          <ShieldCheck data-icon="inline-start" />
          Permissions
        </Button>
      </div>
      <Layout colorScheme={colorScheme} />
    </main>
  )
}
