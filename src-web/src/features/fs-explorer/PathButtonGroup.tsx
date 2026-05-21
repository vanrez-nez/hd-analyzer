import { ArrowLeftIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { ButtonGroup } from "@/components/ui/button-group"

type PathButtonGroupProps = {
  disabled?: boolean
  path?: string
  rootPath?: string
  rootLabel?: string
  labelMaxLength?: number
  onNavigate: (path: string) => void
  onBackToRoot: () => void
}

const DEFAULT_LABEL_MAX_LENGTH = 45

export function PathButtonGroup({
  disabled = false,
  path,
  rootPath,
  rootLabel,
  labelMaxLength = DEFAULT_LABEL_MAX_LENGTH,
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
          disabled={disabled}
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
            className="min-w-0 max-w-none"
            disabled={disabled}
            key={part.path}
            title={part.label}
            onClick={() => onNavigate(part.path)}
          >
            {truncateMiddle(part.label, labelMaxLength)}
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

export function truncateMiddle(value: string, maxLength: number) {
  if (value.length <= maxLength) {
    return value
  }

  if (maxLength <= 3) {
    return value.slice(0, Math.max(0, maxLength))
  }

  const available = maxLength - 3
  const headLength = Math.ceil(available / 2)
  const tailLength = Math.floor(available / 2)
  return `${value.slice(0, headLength)}...${value.slice(value.length - tailLength)}`
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
