import { HardDrive, Folder, FileIcon } from "lucide-react"

import { Spinner } from "@/components/ui/spinner"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import type { DirectoryListingDto, DriveDto, PathNodeDto } from "./types"

type ExplorerTableProps = {
  volumes: DriveDto[]
  listing?: DirectoryListingDto
  loadingPath?: string
  onOpenVolume: (volume: DriveDto) => void
  onOpenNode: (node: PathNodeDto) => void
}

export function ExplorerTable({
  volumes,
  listing,
  loadingPath,
  onOpenVolume,
  onOpenNode,
}: ExplorerTableProps) {
  const rows = listing?.children ?? []

  return (
    <div className="min-h-0 overflow-auto rounded-md border">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Name</TableHead>
            <TableHead className="w-28">Kind</TableHead>
            <TableHead className="w-36 text-right">Size</TableHead>
            <TableHead className="w-32">State</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {!listing
            ? volumes.map((volume) => (
                <TableRow className="cursor-pointer" key={volume.id} onClick={() => onOpenVolume(volume)}>
                  <TableCell>
                    <span className="flex min-w-0 items-center gap-2">
                      <HardDrive data-icon="inline-start" />
                      <span className="truncate">{volume.label}</span>
                    </span>
                  </TableCell>
                  <TableCell>volume</TableCell>
                  <TableCell className="text-right">{formatBytes(volume.usedSpace)}</TableCell>
                  <TableCell>{volume.fileSystem || "ready"}</TableCell>
                </TableRow>
              ))
            : rows.map((node) => (
                <TableRow
                  className={node.kind === "directory" ? "cursor-pointer" : undefined}
                  key={node.path}
                  onClick={() => {
                    if (node.kind === "directory") onOpenNode(node)
                  }}
                >
                  <TableCell>
                    <span className="flex min-w-0 items-center gap-2">
                      {node.kind === "directory" ? <Folder data-icon="inline-start" /> : <FileIcon data-icon="inline-start" />}
                      <span className="truncate">{node.name}</span>
                    </span>
                  </TableCell>
                  <TableCell>{node.kind}</TableCell>
                  <TableCell className="text-right">{formatNodeSize(node)}</TableCell>
                  <TableCell>
                    <span className="flex items-center gap-2">
                      {isWorking(node.state) || loadingPath === node.path ? <Spinner /> : null}
                      <span>{node.state}</span>
                    </span>
                  </TableCell>
                </TableRow>
              ))}
        </TableBody>
      </Table>
    </div>
  )
}

function isWorking(state: string) {
  return state === "queued" || state === "working" || state === "partial" || state === "stale"
}

function formatNodeSize(node: PathNodeDto) {
  if (node.state !== "complete") return ""
  return formatBytes(node.size)
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B"
  const units = ["B", "KB", "MB", "GB", "TB"]
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value.toFixed(value >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`
}
