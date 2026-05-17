import { ArrowLeftIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { ButtonGroup } from "@/components/ui/button-group"

type PathButtonGroupProps = {
  path?: string
  rootPath?: string
  rootLabel?: string
  onNavigate: (path: string) => void
  onBackToRoot: () => void
}

export function PathButtonGroup({
  path,
  rootPath,
  rootLabel,
  onNavigate,
  onBackToRoot,
}: PathButtonGroupProps) {
  if (!path || !rootPath || !rootLabel) {
    return null
  }

  const parts = buildPathSegments(path, rootPath, rootLabel)
  const backPath = getBackPath(path, rootPath)

  return (
    <ButtonGroup className="max-w-full overflow-hidden" aria-label="Path navigation">
      <ButtonGroup>
        <Button
          type="button"
          variant="outline"
          size="icon-sm"
          aria-label="Back"
          onClick={() => {
            if (backPath) {
              onNavigate(backPath)
              return
            }
            onBackToRoot()
          }}
        >
          <ArrowLeftIcon data-icon="inline-start" />
        </Button>
      </ButtonGroup>
      <ButtonGroup className="min-w-0 max-w-full overflow-hidden">
        {parts.map((part) => (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="min-w-0 max-w-48 truncate"
            key={part.path}
            onClick={() => onNavigate(part.path)}
          >
            {part.label}
          </Button>
        ))}
      </ButtonGroup>
    </ButtonGroup>
  )
}

export function buildPathSegments(path: string, rootPath: string, rootLabel: string) {
  const normalizedPath = normalizePath(path)
  const normalizedRoot = normalizePath(rootPath)
  const displayRootLabel = volumeNameOnly(rootLabel, normalizedRoot)

  if (!isInsideRoot(normalizedPath, normalizedRoot)) {
    return [{ label: displayRootLabel, path: normalizedRoot }]
  }

  if (normalizedPath === normalizedRoot) {
    return [{ label: displayRootLabel, path: normalizedRoot }]
  }

  const relative = normalizedPath.slice(normalizedRoot.length).replace(/^\/+/, "")
  const rawParts = relative.split("/").filter(Boolean)
  const parts = rawParts.map((label, index) => {
    const childPath = rawParts.slice(0, index + 1).join("/")
    return {
      label,
      path: normalizedRoot === "/" ? `/${childPath}` : `${normalizedRoot}/${childPath}`,
    }
  })

  return [{ label: displayRootLabel, path: normalizedRoot }, ...parts]
}

export function getBackPath(path: string, rootPath: string) {
  const normalizedPath = normalizePath(path)
  const normalizedRoot = normalizePath(rootPath)

  if (!isInsideRoot(normalizedPath, normalizedRoot) || normalizedPath === normalizedRoot) {
    return undefined
  }

  const relative = normalizedPath.slice(normalizedRoot.length).replace(/^\/+/, "")
  const rawParts = relative.split("/").filter(Boolean)
  if (rawParts.length <= 1) {
    return normalizedRoot
  }

  const parentPath = rawParts.slice(0, -1).join("/")
  return normalizedRoot === "/" ? `/${parentPath}` : `${normalizedRoot}/${parentPath}`
}

function normalizePath(path: string) {
  const normalized = path.replaceAll("\\", "/").replace(/\/+$/, "")
  return normalized || "/"
}

function volumeNameOnly(label: string, rootPath: string) {
  const trimmed = label.trim()
  const suffixMatch = trimmed.match(/^(.*?)\s+\((.*)\)$/)
  if (suffixMatch?.[2] && normalizePath(suffixMatch[2]) === rootPath) {
    return suffixMatch[1].trim() || lastPathPart(rootPath)
  }

  if (normalizePath(trimmed) === rootPath || trimmed.startsWith("/")) {
    return lastPathPart(rootPath)
  }

  return trimmed || lastPathPart(rootPath)
}

function lastPathPart(path: string) {
  if (path === "/") {
    return "/"
  }
  return path.split("/").filter(Boolean).at(-1) ?? path
}

function isInsideRoot(path: string, rootPath: string) {
  return rootPath === "/"
    ? path.startsWith("/")
    : path === rootPath || path.startsWith(`${rootPath}/`)
}
