import { AlertCircle, HardDrive, Play, ShieldCheck } from "lucide-react"
import type { DriveDto } from "@/api"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"

type DriveSelectionProps = {
  drives: DriveDto[]
  selectedDriveId?: string
  loading: boolean
  error?: string
  permissionGranted?: boolean
  permissionChecking: boolean
  permissionMessage?: string
  onSelectDrive: (driveId: string) => void
  onRequestPermissions: () => void
  onStartScan: () => void
}

const formatBytes = (value: number) =>
  new Intl.NumberFormat(undefined, {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value)

export function DriveSelection({
  drives,
  selectedDriveId,
  loading,
  error,
  permissionGranted,
  permissionChecking,
  permissionMessage,
  onSelectDrive,
  onRequestPermissions,
  onStartScan,
}: DriveSelectionProps) {
  const selectedDrive = drives.find((drive) => drive.id === selectedDriveId)
  const scanDisabled = !selectedDrive || loading || permissionChecking || permissionGranted !== true
  const permissionsDisabled = loading || permissionChecking || permissionGranted === true

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">HD Analyzer</h1>
          <p className="text-sm text-muted-foreground">Choose a volume to inspect disk usage.</p>
        </div>
        <div className="flex flex-wrap justify-end gap-2">
          <Button variant="secondary" disabled={permissionsDisabled} onClick={onRequestPermissions}>
            <ShieldCheck className="h-4 w-4" />
            {permissionChecking ? "Checking..." : "Permissions"}
          </Button>
          <Button disabled={scanDisabled} onClick={onStartScan}>
            <Play className="h-4 w-4" />
            Scan
          </Button>
        </div>
      </div>

      {error ? (
        <Alert className="border-destructive/60">
          <AlertCircle className="mr-2 inline h-4 w-4" />
          <AlertTitle>Action failed</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {permissionMessage && permissionGranted === false ? (
        <Alert>
          <AlertCircle className="mr-2 inline h-4 w-4" />
          <AlertTitle>Scan unavailable</AlertTitle>
          <AlertDescription>{permissionMessage}</AlertDescription>
        </Alert>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <HardDrive className="h-4 w-4 text-primary" />
            Volumes
          </CardTitle>
        </CardHeader>
        <CardContent>
          {loading ? (
            <p className="text-sm text-muted-foreground">Loading drives...</p>
          ) : drives.length === 0 ? (
            <Alert>
              <AlertTitle>No drives found</AlertTitle>
              <AlertDescription>No mounted drives were detected on this machine.</AlertDescription>
            </Alert>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Volume</TableHead>
                  <TableHead>Filesystem</TableHead>
                  <TableHead className="text-right">Used</TableHead>
                  <TableHead className="text-right">Free</TableHead>
                  <TableHead className="text-right">Total</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {drives.map((drive) => (
                  <TableRow
                    key={drive.id}
                    className={drive.id === selectedDriveId ? "bg-muted" : undefined}
                    tabIndex={0}
                    onClick={() => onSelectDrive(drive.id)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault()
                        onSelectDrive(drive.id)
                      }
                    }}
                  >
                    <TableCell>
                      <div className="min-w-0">
                        <div className="truncate font-medium">{drive.label}</div>
                        <div className="truncate text-xs text-muted-foreground">{drive.mountPoint}</div>
                      </div>
                    </TableCell>
                    <TableCell>
                      <Badge>{drive.fileSystem || "unknown"}</Badge>
                    </TableCell>
                    <TableCell className="text-right">{formatBytes(drive.usedSpace)}</TableCell>
                    <TableCell className="text-right">{formatBytes(drive.availableSpace)}</TableCell>
                    <TableCell className="text-right">{formatBytes(drive.totalSpace)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </section>
  )
}
