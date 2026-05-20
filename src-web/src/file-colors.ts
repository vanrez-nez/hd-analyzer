import type { ColorScheme } from "@/lib/use-system-color-scheme"

export type FileColorGroup = "file" | "folder" | "audio" | "video" | "document"

type FileColorInput = {
  kind: "volume" | "directory" | "file" | "other"
  label: string
  path: string
}

const AUDIO_EXTENSIONS = new Set([
  "aac",
  "aif",
  "aiff",
  "caf",
  "flac",
  "m4a",
  "mid",
  "midi",
  "mp3",
  "ogg",
  "opus",
  "wav",
  "wma",
])

const VIDEO_EXTENSIONS = new Set([
  "3gp",
  "avi",
  "flv",
  "m4v",
  "mkv",
  "mov",
  "mp4",
  "mpeg",
  "mpg",
  "webm",
  "wmv",
])

const DOCUMENT_EXTENSIONS = new Set([
  "csv",
  "doc",
  "docx",
  "key",
  "md",
  "numbers",
  "odp",
  "ods",
  "odt",
  "pages",
  "pdf",
  "ppt",
  "pptx",
  "rtf",
  "tsv",
  "txt",
  "xls",
  "xlsx",
])

export const FILE_GROUP_COLORS: Record<ColorScheme, Record<FileColorGroup, string>> = {
  light: {
    folder: "hsl(205 46% 42% / 0.58)",
    audio: "hsl(150 36% 38% / 0.56)",
    video: "hsl(8 48% 42% / 0.56)",
    document: "hsl(38 48% 43% / 0.56)",
    file: "hsl(220 10% 42% / 0.50)",
  },
  dark: {
    folder: "hsl(205 62% 58% / 0.64)",
    audio: "hsl(150 48% 52% / 0.62)",
    video: "hsl(8 66% 60% / 0.62)",
    document: "hsl(38 72% 58% / 0.62)",
    file: "hsl(220 12% 66% / 0.54)",
  },
}

export function fileExtension(value: string) {
  const name = value.split(/[\\/]/).filter(Boolean).at(-1) ?? value
  const dotIndex = name.lastIndexOf(".")
  if (dotIndex <= 0 || dotIndex === name.length - 1) {
    return "extensionless"
  }

  return name.slice(dotIndex + 1).toLowerCase()
}

export function fileColorGroupForInput(item: FileColorInput): FileColorGroup {
  switch (item.kind) {
    case "volume":
    case "directory":
      return "folder"
    case "file":
      return fileColorGroupForExtension(fileExtension(item.label || item.path))
    case "other":
      return "file"
  }
}

export function fileColorGroupForExtension(extension: string): FileColorGroup {
  if (AUDIO_EXTENSIONS.has(extension)) {
    return "audio"
  }

  if (VIDEO_EXTENSIONS.has(extension)) {
    return "video"
  }

  if (DOCUMENT_EXTENSIONS.has(extension)) {
    return "document"
  }

  return "file"
}

export function fileColorForGroup(group: FileColorGroup, colorScheme: ColorScheme) {
  return FILE_GROUP_COLORS[colorScheme][group]
}
