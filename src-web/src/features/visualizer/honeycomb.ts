import {
  approximateInscribedCircle,
  distanceToPolygonEdges,
  insetConvexPolygon,
  polygonAbsArea,
} from "./clipper"
import type { VisualizerLayout, VisualizerLayoutCell, VisualizerPoint } from "./voronoi"

export const HONEYCOMB_MIN_CELL_AREA = 1
export const HONEYCOMB_MIN_SEPARATION = 1
export const HONEYCOMB_MAX_SEPARATION = 3
export const HONEYCOMB_MIN_SMOOTH = 0.35
export const HONEYCOMB_MAX_SMOOTH = 1
export const HONEYCOMB_MIN_RESAMPLE_SPACING = 3
export const HONEYCOMB_MAX_RESAMPLE_SPACING = 6
export const HONEYCOMB_MIN_RESAMPLE_POINTS = 4
export const HONEYCOMB_MAX_RESAMPLE_POINTS = 24
export const HONEYCOMB_SMALL_RADIUS = 8
export const HONEYCOMB_LARGE_RADIUS = 48
export const HONEYCOMB_DEBUG_INSET = false

const SMOOTH_EPSILON = 0.000001

type HoneycombOptions = {
  minCellArea?: number
  separation?: number
  smooth?: number
}

type ResolvedHoneycombOptions = {
  minCellArea: number
  maxSeparation: number
  maxSmooth: number
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
}

function drawHoneycomb(
  context: CanvasRenderingContext2D,
  layout: VisualizerLayout,
  options: ResolvedHoneycombOptions,
) {
  context.save()
  layout.cells.forEach((cell) => drawHoneycombCell(context, cell, options))
  context.restore()
}

function drawHoneycombCell(
  context: CanvasRenderingContext2D,
  cell: VisualizerLayoutCell,
  options: ResolvedHoneycombOptions,
) {
  const area = polygonAbsArea(cell.polygon)
  if (area <= 0) {
    return
  }

  if (area < options.minCellArea) {
    drawInscribedCircle(context, cell)
    return
  }

  const style = resolveCellStyle(area, options)
  const inset = insetConvexPolygon(cell.polygon, style.separation)
  if (!inset || polygonAbsArea(inset) < options.minCellArea) {
    drawInscribedCircle(context, cell)
    return
  }

  if (HONEYCOMB_DEBUG_INSET) {
    drawInsetDebug(context, cell.polygon, inset, style.separation)
  }

  drawSmoothPolygon(context, inset, cell.fillColor, style.smooth, style.resampleSpacing)
}

function drawInscribedCircle(
  context: CanvasRenderingContext2D,
  cell: VisualizerLayoutCell,
  circle = approximateInscribedCircle(cell.polygon),
) {
  if (circle.radius <= 0.2) {
    return
  }

  context.save()
  context.beginPath()
  context.arc(circle.center[0], circle.center[1], circle.radius, 0, Math.PI * 2)
  context.fillStyle = cell.fillColor
  context.globalAlpha = 0.92
  context.fill()
  context.strokeStyle = "hsl(0 0% 100% / 0.28)"
  context.lineWidth = 1
  context.stroke()
  context.restore()
}

function drawSmoothPolygon(
  context: CanvasRenderingContext2D,
  polygon: VisualizerPoint[],
  fillStyle: string,
  smooth: number,
  resampleSpacing: number,
) {
  const points = cleanDrawPolygon(polygon)
  if (points.length < 3) {
    return
  }

  if (smooth <= 0.01) {
    drawLinearPolygon(context, points, fillStyle)
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
  context.globalAlpha = 0.92
  context.fill()
  context.strokeStyle = "hsl(0 0% 100% / 0.24)"
  context.lineWidth = 1
  context.stroke()
  context.restore()
}

function drawLinearPolygon(context: CanvasRenderingContext2D, polygon: VisualizerPoint[], fillStyle: string) {
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
  context.globalAlpha = 0.92
  context.fill()
  context.strokeStyle = "hsl(0 0% 100% / 0.24)"
  context.lineWidth = 1
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
    minCellArea: Math.max(0, options.minCellArea ?? HONEYCOMB_MIN_CELL_AREA),
    maxSeparation,
    maxSmooth: clamp(options.smooth ?? HONEYCOMB_MAX_SMOOTH, 0, 1),
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
