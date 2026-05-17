import { ShieldCheck } from "lucide-react"
import { Button } from "@/components/ui/button"

type DriveSelectionProps = {
  permissionChecking: boolean
  onRequestPermissions: () => void
}

export function DriveSelection({ permissionChecking, onRequestPermissions }: DriveSelectionProps) {
  return (
    <Button variant="secondary" disabled={permissionChecking} onClick={onRequestPermissions}>
      <ShieldCheck data-icon="inline-start" />
      Permissions
    </Button>
  )
}
