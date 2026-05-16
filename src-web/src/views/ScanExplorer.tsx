import { AlertCircle, ArrowLeft, FolderOpen, RefreshCw } from "lucide-react"
import type { DirectoryEntryDto, ScanSessionDto } from "@/api"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Progress } from "@/components/ui/progress"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"

type ScanExplorerProps = {
  session: ScanSessionDto
  currentPath: string
  entries: DirectoryEntryDto[]
  loading: boolean
  error?: string
  onBackToDrives: () => void
  onRefreshEntries: () => void
}

const formatBytes = (value: number) =>
  new Intl.NumberFormat(undefined, {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value)

export function ScanExplorer({
  session,
  currentPath,
  entries,
  loading,
  error,
  onBackToDrives,
  onRefreshEntries,
}: ScanExplorerProps) {
  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">Scan Explorer</h1>
          <p className="max-w-3xl truncate text-sm text-muted-foreground">{currentPath}</p>
        </div>
        <div className="flex gap-2">
          <Button variant="secondary" onClick={onBackToDrives}>
            <ArrowLeft className="h-4 w-4" />
            Drives
          </Button>
          <Button onClick={onRefreshEntries}>
            <RefreshCw className="h-4 w-4" />
            Refresh
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

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <FolderOpen className="h-4 w-4 text-primary" />
            {session.status === "scanning" ? "Scanning" : "Results"}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {session.status === "scanning" || loading ? <Progress value={loading ? 35 : 100} /> : null}
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Directory</TableHead>
                <TableHead className="text-right">Size</TableHead>
                <TableHead className="text-right">Share</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {entries.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={3} className="text-muted-foreground">
                    No entries are available yet. Refresh after the scan completes.
                  </TableCell>
                </TableRow>
              ) : (
                entries.map((entry) => (
                  <TableRow key={entry.path}>
                    <TableCell className="max-w-[620px] truncate">{entry.displayName}</TableCell>
                    <TableCell className="text-right">{formatBytes(entry.size)}</TableCell>
                    <TableCell className="text-right">{(entry.share * 100).toFixed(1)}%</TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </section>
  )
}
