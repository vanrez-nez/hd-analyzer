import {
  approximateInscribedCircle,
  distanceToPolygonEdges,
  insetConvexPolygon,
  polygonAbsArea,
} from "./clipper"
import { adjustColorForTheme } from "@/file-colors"
import type { ColorScheme } from "@/lib/use-system-color-scheme"
import type { VisualizerLayout, VisualizerLayoutCell, VisualizerPoint } from "./voronoi"

export const HONEYCOMB_MIN_CELL_AREA = 1
export const HONEYCOMB_MIN_SEPARATION = 2
export const HONEYCOMB_MAX_SEPARATION = 4
export const HONEYCOMB_MIN_SMOOTH = 0.35
export const HONEYCOMB_MAX_SMOOTH = 1
export const HONEYCOMB_MIN_RESAMPLE_SPACING = 3
export const HONEYCOMB_MAX_RESAMPLE_SPACING = 6
export const HONEYCOMB_MIN_RESAMPLE_POINTS = 4
export const HONEYCOMB_MAX_RESAMPLE_POINTS = 24
export const HONEYCOMB_SMALL_RADIUS = 8
export const HONEYCOMB_LARGE_RADIUS = 48
export const HONEYCOMB_DEBUG_INSET = false
export const LABEL_COLOR = "hsl(0 0% 100% / 0.86)"
export const LABEL_FONT_SIZE_MIN = 9
export const LABEL_FONT_SIZE_MAX = 13
export const LABEL_VISIBILITY_THRESHOLD = 0.006
export const LABEL_LENGTH_TRUNCATE = 28

const SMOOTH_EPSILON = 0.000001
const LABEL_AREA_RATIO = 0.82
const LABEL_HEIGHT_RATIO = 0.78
const LABEL_LINE_HEIGHT = 1.15
const HONEYCOMB_FALLBACK_STROKE = "hsl(0 0% 100% / 0.24)"
const HONEYCOMB_BORDER_LIGHTNESS_DELTA = 10
const HONEYCOMB_SELECTED_FILL_LIGHTNESS_DELTA = 10
const HONEYCOMB_SELECTED_BORDER_LIGHTNESS_DELTA = 24
const HONEYCOMB_SELECTED_STROKE_WIDTH = 2

type HoneycombOptions = {
  colorScheme?: ColorScheme
  minCellArea?: number
  selectedItemIds?: readonly string[]
  selectionProgressById?: ReadonlyMap<string, number>
  separation?: number
  smooth?: number
}

type ResolvedHoneycombOptions = {
  colorScheme: ColorScheme
  minCellArea: number
  maxSeparation: number
  maxSmooth: number
  selectedItemIds: Set<string>
  selectionProgressById: ReadonlyMap<string, number>
}

type HoneycombCellStyle = {
  separation: number
  smooth: number
  resampleSpacing: number
}

export class Honeycomb {
  draw(context: CanvasRenderingContext2D, layout: VisualizerLayout, options: HoneycombOptions = {}) {
    drawHoneycomb(context, layout, resolveOptions(options))
  }

  hitTest(layout: VisualizerLayout, point: VisualizerPoint, options: HoneycombOptions = {}) {
    return hitTestHoneycomb(layout, point, resolveOptions(options))
  }
}

function drawHoneycomb(
  context: CanvasRenderingContext2D,
  layout: VisualizerLayout,
  options: ResolvedHoneycombOptions,
) {
  context.save()
  const layoutCircleArea = Math.PI * layout.circle.radius ** 2
  layout.cells.forEach((cell) => drawHoneycombCell(context, cell, options, layoutCircleArea))
  context.restore()
}

function hitTestHoneycomb(
  layout: VisualizerLayout,
  point: VisualizerPoint,
  options: ResolvedHoneycombOptions,
) {
  for (let index = layout.cells.length - 1; index >= 0; index -= 1) {
    const cell = layout.cells[index]
    const geometry = resolveCellHitGeometry(cell, options)
    if (!geometry) {
      continue
    }

    if (geometry.kind === "circle" && pointInCircle(point, geometry.center, geometry.radius)) {
      return cell
    }

    if (geometry.kind === "polygon" && pointInPolygon(point, geometry.polygon)) {
      return cell
    }
  }

  return undefined
}

function drawHoneycombCell(
  context: CanvasRenderingContext2D,
  cell: VisualizerLayoutCell,
  options: ResolvedHoneycombOptions,
  layoutCircleArea: number,
) {
  const area = polygonAbsArea(cell.polygon)
  if (area <= 0) {
    return
  }

  const selectionProgress = selectedCellProgress(cell, options)

  if (area < options.minCellArea) {
    drawInscribedCircle(context, cell, options, selectionProgress, undefined, layoutCircleArea)
    return
  }

  const style = resolveCellStyle(area, options)
  const inset = insetConvexPolygon(cell.polygon, style.separation)
  if (!inset || polygonAbsArea(inset) < options.minCellArea) {
    drawInscribedCircle(context, cell, options, selectionProgress, undefined, layoutCircleArea)
    return
  }

  if (HONEYCOMB_DEBUG_INSET) {
    drawInsetDebug(context, cell.polygon, inset, style.separation)
  }

  drawSmoothCell(context, cell, options, inset, style, selectionProgress)
  drawCellLabel(context, cell, approximateInscribedCircle(inset), layoutCircleArea)
}

function resolveCellHitGeometry(cell: VisualizerLayoutCell, options: ResolvedHoneycombOptions) {
  const area = polygonAbsArea(cell.polygon)
  if (area <= 0) {
    return undefined
  }

  if (area < options.minCellArea) {
    const circle = approximateInscribedCircle(cell.polygon)
    return circle.radius > 0.2
      ? {
          kind: "circle" as const,
          center: circle.center,
          radius: circle.radius,
        }
      : undefined
  }

  const style = resolveCellStyle(area, options)
  const inset = insetConvexPolygon(cell.polygon, style.separation)
  if (!inset || polygonAbsArea(inset) < options.minCellArea) {
    const circle = approximateInscribedCircle(cell.polygon)
    return circle.radius > 0.2
      ? {
          kind: "circle" as const,
          center: circle.center,
          radius: circle.radius,
        }
      : undefined
  }

  return {
    kind: "polygon" as const,
    polygon: inset,
  }
}

function resolveCellDrawStyle(
  cell: VisualizerLayoutCell,
  options: ResolvedHoneycombOptions,
  selected: boolean,
) {
  if (!selected) {
    return {
      fill: cell.fillColor,
      stroke: borderColorForCell(cell.fillColor, options.colorScheme),
      lineWidth: 1,
    }
  }

  return {
    fill: selectedFillColorForCell(cell.fillColor, options.colorScheme),
    stroke: selectedBorderColorForCell(cell.fillColor, options.colorScheme),
    lineWidth: HONEYCOMB_SELECTED_STROKE_WIDTH,
  }
}

function drawSmoothCell(
  context: CanvasRenderingContext2D,
  cell: VisualizerLayoutCell,
  options: ResolvedHoneycombOptions,
  inset: VisualizerPoint[],
  style: HoneycombCellStyle,
  selectionProgress: number | undefined,
) {
  if (selectionProgress === undefined || selectionProgress >= 1) {
    const cellStyle = resolveCellDrawStyle(cell, options, selectionProgress !== undefined)
    drawSmoothPolygon(
      context,
      inset,
      cellStyle.fill,
      cellStyle.stroke,
      style.smooth,
      style.resampleSpacing,
      cellStyle.lineWidth,
    )
    return
  }

  const baseStyle = resolveCellDrawStyle(cell, options, false)
  drawSmoothPolygon(
    context,
    inset,
    baseStyle.fill,
    baseStyle.stroke,
    style.smooth,
    style.resampleSpacing,
    baseStyle.lineWidth,
  )

  const selectedStyle = resolveCellDrawStyle(cell, options, true)
  drawSmoothPolygon(
    context,
    inset,
    selectedStyle.fill,
    selectedStyle.stroke,
    style.smooth,
    style.resampleSpacing,
    selectedStyle.lineWidth,
    0.92 * selectionProgress,
  )
}

function drawInscribedCircle(
  context: CanvasRenderingContext2D,
  cell: VisualizerLayoutCell,
  options: ResolvedHoneycombOptions,
  selectionProgress: number | undefined,
  circle = approximateInscribedCircle(cell.polygon),
  layoutCircleArea = Infinity,
) {
  if (circle.radius <= 0.2) {
    return
  }

  if (selectionProgress === undefined || selectionProgress >= 1) {
    const cellStyle = resolveCellDrawStyle(cell, options, selectionProgress !== undefined)
    drawCircleShape(context, circle, cellStyle, 0.92)
    drawCellLabel(context, cell, circle, layoutCircleArea)
    return
  }

  drawCircleShape(context, circle, resolveCellDrawStyle(cell, options, false), 0.92)
  drawCircleShape(context, circle, resolveCellDrawStyle(cell, options, true), 0.92 * selectionProgress)
  drawCellLabel(context, cell, circle, layoutCircleArea)
}

function drawCircleShape(
  context: CanvasRenderingContext2D,
  circle: { center: VisualizerPoint; radius: number },
  cellStyle: { fill: string; lineWidth: number; stroke: string },
  alpha: number,
) {
  context.save()
  context.beginPath()
  context.arc(circle.center[0], circle.center[1], circle.radius, 0, Math.PI * 2)
  context.fillStyle = cellStyle.fill
  context.globalAlpha = alpha
  context.fill()
  context.strokeStyle = cellStyle.stroke
  context.lineWidth = cellStyle.lineWidth
  context.stroke()
  context.restore()
}

function drawCellLabel(
  context: CanvasRenderingContext2D,
  cell: VisualizerLayoutCell,
  circle: { center: VisualizerPoint; radius: number },
  layoutCircleArea: number,
) {
  const circleArea = Math.PI * circle.radius ** 2
  if (
    circle.radius <= 0 ||
    !Number.isFinite(layoutCircleArea) ||
    circleArea / layoutCircleArea < LABEL_VISIBILITY_THRESHOLD
  ) {
    return
  }

  const fontSize = clamp(circle.radius * 0.32, LABEL_FONT_SIZE_MIN, LABEL_FONT_SIZE_MAX)
  const lineHeight = fontSize * LABEL_LINE_HEIGHT
  const totalHeight = lineHeight * 2
  const availableHeight = circle.radius * 2 * LABEL_HEIGHT_RATIO
  if (totalHeight > availableHeight) {
    return
  }

  const name = truncateMiddle(cell.label, LABEL_LENGTH_TRUNCATE)
  const size = formatBytes(cell.size)
  const availableWidth = circle.radius * 2 * LABEL_AREA_RATIO
  context.save()
  context.font = `${fontSize}px system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`
  const nameWidth = context.measureText(name).width
  const sizeWidth = context.measureText(size).width
  if (nameWidth > availableWidth || sizeWidth > availableWidth) {
    context.restore()
    return
  }

  context.fillStyle = LABEL_COLOR
  context.globalAlpha = 1
  context.textAlign = "center"
  context.textBaseline = "middle"
  context.fillText(name, circle.center[0], circle.center[1] - lineHeight / 2)
  context.fillText(size, circle.center[0], circle.center[1] + lineHeight / 2)
  context.restore()
}

function drawSmoothPolygon(
  context: CanvasRenderingContext2D,
  polygon: VisualizerPoint[],
  fillStyle: string,
  strokeStyle: string,
  smooth: number,
  resampleSpacing: number,
  lineWidth: number,
  alpha = 0.92,
) {
  const points = cleanDrawPolygon(polygon)
  if (points.length < 3) {
    return
  }

  if (smooth <= 0.01) {
    drawLinearPolygon(context, points, fillStyle, strokeStyle, lineWidth, alpha)
    return
  }

  const resampled = resampleClosedPolygonByArcLength(
    points,
    resampleSpacing,
    HONEYCOMB_MIN_RESAMPLE_POINTS,
    HONEYCOMB_MAX_RESAMPLE_POINTS,
  )
  const tension = Math.min(0.95, Math.max(0, smooth))
  const handle = tension * 0.5
  const corners = resampled.map((current, index) => {
    const previous = resampled[(index + resampled.length - 1) % resampled.length]
    const next = resampled[(index + 1) % resampled.length]

    return {
      incoming: lerpPoint(current, previous, handle),
      outgoing: lerpPoint(current, next, handle),
    }
  })
  const firstCorner = corners[0]

  context.save()
  context.beginPath()
  context.moveTo(firstCorner.outgoing[0], firstCorner.outgoing[1])

  for (let index = 1; index < resampled.length; index += 1) {
    const current = resampled[index]
    const corner = corners[index]

    context.lineTo(corner.incoming[0], corner.incoming[1])
    context.quadraticCurveTo(current[0], current[1], corner.outgoing[0], corner.outgoing[1])
  }

  context.lineTo(firstCorner.incoming[0], firstCorner.incoming[1])
  context.quadraticCurveTo(resampled[0][0], resampled[0][1], firstCorner.outgoing[0], firstCorner.outgoing[1])
  context.closePath()
  context.fillStyle = fillStyle
  context.globalAlpha = alpha
  context.fill()
  context.strokeStyle = strokeStyle
  context.lineWidth = lineWidth
  context.stroke()
  context.restore()
}

function drawLinearPolygon(
  context: CanvasRenderingContext2D,
  polygon: VisualizerPoint[],
  fillStyle: string,
  strokeStyle: string,
  lineWidth: number,
  alpha = 0.92,
) {
  const points = cleanDrawPolygon(polygon)
  if (points.length < 3) {
    return
  }

  context.save()
  context.beginPath()
  points.forEach(([x, y], index) => {
    if (index === 0) {
      context.moveTo(x, y)
      return
    }

    context.lineTo(x, y)
  })
  context.closePath()
  context.fillStyle = fillStyle
  context.globalAlpha = alpha
  context.fill()
  context.strokeStyle = strokeStyle
  context.lineWidth = lineWidth
  context.stroke()
  context.restore()
}

function drawInsetDebug(
  context: CanvasRenderingContext2D,
  original: VisualizerPoint[],
  inset: VisualizerPoint[],
  separation: number,
) {
  const points = cleanDrawPolygon(inset)
  if (points.length < 3) {
    return
  }

  context.save()
  context.beginPath()
  points.forEach(([x, y], index) => {
    if (index === 0) {
      context.moveTo(x, y)
      return
    }

    context.lineTo(x, y)
  })
  context.closePath()
  context.strokeStyle = "hsl(180 100% 55% / 0.8)"
  context.lineWidth = 1
  context.stroke()

  points.forEach((point) => {
    const isBad = distanceToPolygonEdges(point, original) < separation * 0.75
    if (!isBad) {
      return
    }

    context.beginPath()
    context.arc(point[0], point[1], 2.5, 0, Math.PI * 2)
    context.fillStyle = "hsl(0 100% 55% / 0.9)"
    context.fill()
  })

  context.restore()
}

function lerpPoint(from: VisualizerPoint, to: VisualizerPoint, amount: number): VisualizerPoint {
  return [from[0] + (to[0] - from[0]) * amount, from[1] + (to[1] - from[1]) * amount]
}

function cleanDrawPolygon(polygon: VisualizerPoint[]) {
  const points: VisualizerPoint[] = []

  for (const point of polygon) {
    const previous = points[points.length - 1]
    if (!previous || pointDistance(point, previous) > SMOOTH_EPSILON) {
      points.push([point[0], point[1]])
    }
  }

  const first = points[0]
  const last = points[points.length - 1]
  if (first && last && pointDistance(first, last) <= SMOOTH_EPSILON) {
    points.pop()
  }

  return points
}

function pointDistance(left: VisualizerPoint, right: VisualizerPoint) {
  return Math.hypot(left[0] - right[0], left[1] - right[1])
}

function pointInCircle(point: VisualizerPoint, center: VisualizerPoint, radius: number) {
  return pointDistance(point, center) <= radius
}

function pointInPolygon(point: VisualizerPoint, polygon: VisualizerPoint[]) {
  const points = cleanDrawPolygon(polygon)
  if (points.length < 3) {
    return false
  }

  let inside = false
  for (let index = 0, previousIndex = points.length - 1; index < points.length; previousIndex = index, index += 1) {
    const current = points[index]
    const previous = points[previousIndex]
    const crossesY = current[1] > point[1] !== previous[1] > point[1]
    if (!crossesY) {
      continue
    }

    const xAtY =
      ((previous[0] - current[0]) * (point[1] - current[1])) / (previous[1] - current[1]) + current[0]
    if (point[0] < xAtY) {
      inside = !inside
    }
  }

  return inside
}

function polygonPerimeter(points: VisualizerPoint[]) {
  return points.reduce((perimeter, point, index) => {
    return perimeter + pointDistance(point, points[(index + 1) % points.length])
  }, 0)
}

function resampleClosedPolygonByArcLength(
  polygon: VisualizerPoint[],
  spacing: number,
  minPoints: number,
  maxPoints: number,
) {
  const points = cleanDrawPolygon(polygon)
  if (points.length < 3) {
    return points
  }

  const perimeter = polygonPerimeter(points)
  if (perimeter <= SMOOTH_EPSILON) {
    return points
  }

  const targetCount = clampInteger(
    Math.round(perimeter / Math.max(spacing, SMOOTH_EPSILON)),
    minPoints,
    maxPoints,
  )
  const cumulativeLengths = [0]

  for (let index = 0; index < points.length; index += 1) {
    const current = points[index]
    const next = points[(index + 1) % points.length]
    cumulativeLengths.push(cumulativeLengths[index] + pointDistance(current, next))
  }

  return cleanDrawPolygon(
    Array.from({ length: targetCount }, (_, index) => {
      return interpolateOnClosedPerimeter(points, cumulativeLengths, (perimeter * index) / targetCount)
    }),
  )
}

function interpolateOnClosedPerimeter(
  points: VisualizerPoint[],
  cumulativeLengths: number[],
  targetDistance: number,
) {
  const perimeter = cumulativeLengths[cumulativeLengths.length - 1]
  const distance = ((targetDistance % perimeter) + perimeter) % perimeter
  let segmentIndex = 0

  while (segmentIndex + 1 < cumulativeLengths.length - 1 && cumulativeLengths[segmentIndex + 1] < distance) {
    segmentIndex += 1
  }

  const segmentStart = cumulativeLengths[segmentIndex]
  const segmentEnd = cumulativeLengths[segmentIndex + 1]
  const segmentLength = segmentEnd - segmentStart
  if (segmentLength <= SMOOTH_EPSILON) {
    const point = points[segmentIndex]
    return [point[0], point[1]] as VisualizerPoint
  }

  return lerpPoint(
    points[segmentIndex],
    points[(segmentIndex + 1) % points.length],
    (distance - segmentStart) / segmentLength,
  )
}

function resolveOptions(options: HoneycombOptions): ResolvedHoneycombOptions {
  const maxSeparation = Math.max(0, options.separation ?? HONEYCOMB_MAX_SEPARATION)

  return {
    colorScheme: options.colorScheme ?? "light",
    minCellArea: Math.max(0, options.minCellArea ?? HONEYCOMB_MIN_CELL_AREA),
    maxSeparation,
    maxSmooth: clamp(options.smooth ?? HONEYCOMB_MAX_SMOOTH, 0, 1),
    selectedItemIds: new Set(options.selectedItemIds ?? []),
    selectionProgressById: options.selectionProgressById ?? new Map<string, number>(),
  }
}

function resolveCellStyle(area: number, options: ResolvedHoneycombOptions): HoneycombCellStyle {
  const radius = Math.sqrt(area / Math.PI)
  const scale = smoothStep(
    clamp(
      (radius - HONEYCOMB_SMALL_RADIUS) / (HONEYCOMB_LARGE_RADIUS - HONEYCOMB_SMALL_RADIUS),
      0,
      1,
    ),
  )

  return {
    separation: lerpNumber(Math.min(HONEYCOMB_MIN_SEPARATION, options.maxSeparation), options.maxSeparation, scale),
    smooth: lerpNumber(Math.min(HONEYCOMB_MIN_SMOOTH, options.maxSmooth), options.maxSmooth, scale),
    resampleSpacing: lerpNumber(HONEYCOMB_MIN_RESAMPLE_SPACING, HONEYCOMB_MAX_RESAMPLE_SPACING, scale),
  }
}

function smoothStep(value: number) {
  return value * value * (3 - 2 * value)
}

function lerpNumber(from: number, to: number, amount: number) {
  return from + (to - from) * amount
}

function truncateMiddle(value: string, maxLength: number) {
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

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "0 B"
  }

  const units = ["B", "KB", "MB", "GB", "TB"]
  let value = bytes
  let unit = 0
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000
    unit += 1
  }

  const maximumFractionDigits = unit === 0 ? 0 : 2
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits }).format(value)} ${units[unit]}`
}

function selectedCellProgress(cell: VisualizerLayoutCell, options: ResolvedHoneycombOptions) {
  if (options.selectedItemIds.size === 0) {
    return undefined
  }

  let selected = false
  let progress = 0

  for (const itemId of selectedCellCandidateIds(cell)) {
    if (!options.selectedItemIds.has(itemId)) {
      continue
    }

    selected = true
    progress = Math.max(progress, options.selectionProgressById.get(itemId) ?? 1)
  }

  return selected ? clamp(progress, 0, 1) : undefined
}

function selectedCellCandidateIds(cell: VisualizerLayoutCell) {
  return [cell.id, cell.selectionId, ...cell.memberIds]
}

function borderColorForCell(fillStyle: string, colorScheme: ColorScheme) {
  return adjustColorForTheme(fillStyle, colorScheme, HONEYCOMB_BORDER_LIGHTNESS_DELTA) ?? HONEYCOMB_FALLBACK_STROKE
}

function selectedFillColorForCell(fillStyle: string, colorScheme: ColorScheme) {
  return adjustColorForTheme(fillStyle, colorScheme, HONEYCOMB_SELECTED_FILL_LIGHTNESS_DELTA) ?? fillStyle
}

function selectedBorderColorForCell(fillStyle: string, colorScheme: ColorScheme) {
  return (
    adjustColorForTheme(fillStyle, colorScheme, HONEYCOMB_SELECTED_BORDER_LIGHTNESS_DELTA) ??
    HONEYCOMB_FALLBACK_STROKE
  )
}

function clamp(value: number, min: number, max: number) {
  if (!Number.isFinite(value)) {
    return min
  }

  return Math.min(max, Math.max(min, value))
}

function clampInteger(value: number, min: number, max: number) {
  if (!Number.isFinite(value)) {
    return min
  }

  return Math.min(max, Math.max(min, Math.round(value)))
}
