import type { ColorScheme } from "@/lib/use-system-color-scheme"

export type FileColorGroup = "file" | "folder" | "audio" | "video" | "document"
export type VisualizerColorToken =
  | "circleBackground"
  | "circleFrame"
  | "fallbackStroke"
  | "label"
  | "placeholderFill"

type FileColorInput = {
  kind: "volume" | "directory" | "file" | "other"
  label: string
  path: string
}

type HslColor = {
  alpha: number
  hue: number
  lightness: number
  saturation: number
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

const FILE_GROUP_COLOR_VARIABLES: Record<FileColorGroup, string> = {
  folder: "--visualizer-group-folder",
  audio: "--visualizer-group-audio",
  video: "--visualizer-group-video",
  document: "--visualizer-group-document",
  file: "--visualizer-group-file",
}

const VISUALIZER_COLOR_VARIABLES: Record<VisualizerColorToken, string> = {
  circleBackground: "--visualizer-circle-background",
  circleFrame: "--visualizer-circle-frame",
  fallbackStroke: "--visualizer-fallback-stroke",
  label: "--visualizer-label",
  placeholderFill: "--visualizer-placeholder-fill",
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

export function fileColorForGroup(group: FileColorGroup) {
  return readCssColorVariable(FILE_GROUP_COLOR_VARIABLES[group])
}

export function visualizerColor(token: VisualizerColorToken) {
  return readCssColorVariable(VISUALIZER_COLOR_VARIABLES[token])
}

function readCssColorVariable(name: string) {
  if (typeof window === "undefined") {
    return ""
  }

  return window.getComputedStyle(document.documentElement).getPropertyValue(name).trim()
}

export function adjustColorForTheme(value: string, colorScheme: ColorScheme, lightnessAmount: number) {
  const color = parseHslColor(value)
  if (!color) {
    return undefined
  }

  const lightnessAdjustment = colorScheme === "dark" ? lightnessAmount : -lightnessAmount
  const lightness = clamp(color.lightness + lightnessAdjustment, 0, 100)
  return formatHslColor({ ...color, lightness })
}

export function mixHslColors(from: string, to: string, amount: number) {
  const fromColor = parseHslColor(from)
  const toColor = parseHslColor(to)
  if (!fromColor || !toColor) {
    return undefined
  }

  const progress = clamp(amount, 0, 1)
  const hueDelta = shortestHueDelta(fromColor.hue, toColor.hue)

  return formatHslColor({
    alpha: lerpNumber(fromColor.alpha, toColor.alpha, progress),
    hue: normalizeHue(fromColor.hue + hueDelta * progress),
    lightness: lerpNumber(fromColor.lightness, toColor.lightness, progress),
    saturation: lerpNumber(fromColor.saturation, toColor.saturation, progress),
  })
}

function parseHslColor(value: string): HslColor | undefined {
  const match = value
    .trim()
    .match(/^hsl\(\s*([-+]?\d*\.?\d+)\s+([-+]?\d*\.?\d+)%\s+([-+]?\d*\.?\d+)%(?:\s*\/\s*([^)]+?))?\s*\)$/)

  if (!match) {
    return undefined
  }

  const hue = Number.parseFloat(match[1])
  const saturation = Number.parseFloat(match[2])
  const lightness = Number.parseFloat(match[3])
  const alpha = parseAlpha(match[4]?.trim())
  if (![hue, saturation, lightness, alpha].every(Number.isFinite)) {
    return undefined
  }

  return {
    alpha: clamp(alpha, 0, 1),
    hue: normalizeHue(hue),
    lightness: clamp(lightness, 0, 100),
    saturation: clamp(saturation, 0, 100),
  }
}

function parseAlpha(value: string | undefined) {
  if (!value) {
    return 1
  }

  if (value.endsWith("%")) {
    return Number.parseFloat(value) / 100
  }

  return Number.parseFloat(value)
}

function formatHslColor(color: HslColor) {
  return `hsl(${formatNumber(color.hue)} ${formatNumber(color.saturation)}% ${formatNumber(color.lightness)}% / ${formatNumber(color.alpha)})`
}

function formatNumber(value: number) {
  return Number.parseFloat(value.toFixed(3)).toString()
}

function normalizeHue(value: number) {
  return ((value % 360) + 360) % 360
}

function shortestHueDelta(from: number, to: number) {
  return ((to - from + 540) % 360) - 180
}

function lerpNumber(from: number, to: number, amount: number) {
  return from + (to - from) * amount
}

function clamp(value: number, min: number, max: number) {
  if (!Number.isFinite(value)) {
    return min
  }

  return Math.min(max, Math.max(min, value))
}
