import type { VisualizerCellInput } from "./types"

export type FileColorGroup = "file" | "folder" | "audio" | "video" | "document"

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

const FILE_GROUP_COLORS: Record<FileColorGroup, string> = {
  folder: "hsl(205 46% 42% / 0.58)",
  audio: "hsl(150 36% 38% / 0.56)",
  video: "hsl(8 48% 42% / 0.56)",
  document: "hsl(38 48% 43% / 0.56)",
  file: "hsl(220 10% 42% / 0.50)",
}

export function fileExtension(value: string) {
  const name = value.split(/[\\/]/).filter(Boolean).at(-1) ?? value
  const dotIndex = name.lastIndexOf(".")
  if (dotIndex <= 0 || dotIndex === name.length - 1) {
    return "extensionless"
  }

  return name.slice(dotIndex + 1).toLowerCase()
}

export function fileColorGroupForInput(item: VisualizerCellInput): FileColorGroup {
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

export function fileColorForGroup(group: FileColorGroup) {
  return FILE_GROUP_COLORS[group]
}
