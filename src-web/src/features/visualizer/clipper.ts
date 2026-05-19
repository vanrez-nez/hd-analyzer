export type ClipperPoint = [number, number]

export type ClipperCircle = {
  center: ClipperPoint
  radius: number
}

const EPSILON = 0.000001
const INSET_SEARCH_STEPS = 10

export function polygonArea(polygon: ClipperPoint[]) {
  return signedPolygonArea(polygon)
}

export function polygonAbsArea(polygon: ClipperPoint[]) {
  return Math.abs(signedPolygonArea(polygon))
}

export function polygonCentroid(polygon: ClipperPoint[]): ClipperPoint {
  const area = signedPolygonArea(polygon)
  if (Math.abs(area) < EPSILON) {
    return averagePoint(polygon)
  }

  let x = 0
  let y = 0

  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]
    const next = polygon[(index + 1) % polygon.length]
    const cross = current[0] * next[1] - next[0] * current[1]
    x += (current[0] + next[0]) * cross
    y += (current[1] + next[1]) * cross
  }

  const factor = 1 / (6 * area)
  return [x * factor, y * factor]
}

export function insetConvexPolygon(polygon: ClipperPoint[], distance: number) {
  const clean = cleanPolygon(polygon)
  if (clean.length < 3) {
    return undefined
  }

  if (distance <= EPSILON) {
    return clean
  }

  if (!isConvexPolygon(clean)) {
    return undefined
  }

  const originalArea = signedPolygonArea(clean)
  const originalAbsArea = Math.abs(originalArea)
  if (originalAbsArea < EPSILON) {
    return undefined
  }

  const direction = originalArea >= 0 ? 1 : -1
  const inset = insetConvexPolygonAtDistance(clean, direction, originalArea, originalAbsArea, distance)
  if (inset) {
    return inset
  }

  return findLargestInsetPolygon(clean, direction, originalArea, originalAbsArea, distance)
}

export function approximateInscribedCircle(polygon: ClipperPoint[], iterations = 12): ClipperCircle {
  const clean = cleanPolygon(polygon)
  if (clean.length < 3) {
    return {
      center: averagePoint(clean),
      radius: 0,
    }
  }

  const bounds = polygonBounds(clean)
  const candidates: ClipperPoint[] = [
    polygonCentroid(clean),
    [(bounds.minX + bounds.maxX) / 2, (bounds.minY + bounds.maxY) / 2],
  ]

  const gridSize = 4
  for (let xIndex = 0; xIndex <= gridSize; xIndex += 1) {
    for (let yIndex = 0; yIndex <= gridSize; yIndex += 1) {
      candidates.push([
        bounds.minX + ((bounds.maxX - bounds.minX) * xIndex) / gridSize,
        bounds.minY + ((bounds.maxY - bounds.minY) * yIndex) / gridSize,
      ])
    }
  }

  let best = findBestPoint(candidates, clean)
  if (!best) {
    return {
      center: polygonCentroid(clean),
      radius: 0,
    }
  }

  let step = Math.max(bounds.maxX - bounds.minX, bounds.maxY - bounds.minY) / 2

  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const candidates: ClipperPoint[] = []

    for (let x = -1; x <= 1; x += 1) {
      for (let y = -1; y <= 1; y += 1) {
        candidates.push([best.center[0] + x * step, best.center[1] + y * step])
      }
    }

    best = findBestPoint(candidates, clean) ?? best
    step *= 0.5
  }

  return {
    center: best.center,
    radius: Math.max(0, best.radius),
  }
}

export function distanceToPolygonEdges(point: ClipperPoint, polygon: ClipperPoint[]) {
  let distance = Infinity

  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]
    const next = polygon[(index + 1) % polygon.length]
    distance = Math.min(distance, distanceToSegment(point, current, next))
  }

  return Number.isFinite(distance) ? distance : 0
}

export function isConvexPolygon(polygon: ClipperPoint[]) {
  if (polygon.length < 3) {
    return false
  }

  let sign = 0

  for (let index = 0; index < polygon.length; index += 1) {
    const a = polygon[index]
    const b = polygon[(index + 1) % polygon.length]
    const c = polygon[(index + 2) % polygon.length]
    const cross = crossProduct(subtract(b, a), subtract(c, b))

    if (Math.abs(cross) < EPSILON) {
      continue
    }

    const nextSign = Math.sign(cross)
    if (sign !== 0 && nextSign !== sign) {
      return false
    }

    sign = nextSign
  }

  return sign !== 0
}

function insetConvexPolygonAtDistance(
  polygon: ClipperPoint[],
  direction: number,
  originalArea: number,
  originalAbsArea: number,
  distance: number,
) {
  let output = cleanPolygon(polygon)

  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]
    const next = polygon[(index + 1) % polygon.length]
    const edge = shiftedInsetEdge(current, next, direction, distance)
    if (!edge) {
      return undefined
    }

    output = clipPolygonToInsetHalfPlane(output, edge, direction)
    if (output.length < 3) {
      return undefined
    }
  }

  return validateInsetOutput(output, originalArea, originalAbsArea)
}

function findLargestInsetPolygon(
  polygon: ClipperPoint[],
  direction: number,
  originalArea: number,
  originalAbsArea: number,
  distance: number,
) {
  let low = 0
  let high = distance
  let best: ClipperPoint[] | undefined

  for (let step = 0; step < INSET_SEARCH_STEPS; step += 1) {
    const midpoint = (low + high) / 2
    const inset = insetConvexPolygonAtDistance(polygon, direction, originalArea, originalAbsArea, midpoint)

    if (inset) {
      best = inset
      low = midpoint
    } else {
      high = midpoint
    }
  }

  return best
}

function shiftedInsetEdge(
  start: ClipperPoint,
  end: ClipperPoint,
  direction: number,
  distance: number,
) {
  const dx = end[0] - start[0]
  const dy = end[1] - start[1]
  const length = Math.hypot(dx, dy)
  if (length < EPSILON) {
    return undefined
  }

  const normal: ClipperPoint = direction > 0 ? [-dy / length, dx / length] : [dy / length, -dx / length]

  return {
    start: [start[0] + normal[0] * distance, start[1] + normal[1] * distance] as ClipperPoint,
    end: [end[0] + normal[0] * distance, end[1] + normal[1] * distance] as ClipperPoint,
  }
}

function clipPolygonToInsetHalfPlane(
  subject: ClipperPoint[],
  edge: { start: ClipperPoint; end: ClipperPoint },
  direction: number,
) {
  if (subject.length < 3) {
    return []
  }

  const output: ClipperPoint[] = []
  let previous = subject[subject.length - 1]
  let previousInside = isInsideInsetHalfPlane(previous, edge, direction)

  for (const current of subject) {
    const currentInside = isInsideInsetHalfPlane(current, edge, direction)

    if (currentInside) {
      if (!previousInside) {
        const intersection = lineIntersection(previous, current, edge.start, edge.end)
        if (intersection) {
          output.push(intersection)
        }
      }

      output.push(current)
    } else if (previousInside) {
      const intersection = lineIntersection(previous, current, edge.start, edge.end)
      if (intersection) {
        output.push(intersection)
      }
    }

    previous = current
    previousInside = currentInside
  }

  return cleanPolygon(output)
}

function isInsideInsetHalfPlane(
  point: ClipperPoint,
  edge: { start: ClipperPoint; end: ClipperPoint },
  direction: number,
) {
  return direction * crossProduct(subtract(edge.end, edge.start), subtract(point, edge.start)) >= -EPSILON
}

function validateInsetOutput(
  polygon: ClipperPoint[],
  originalArea: number,
  originalAbsArea: number,
) {
  const clean = cleanPolygon(polygon)
  if (clean.length < 3) {
    return undefined
  }

  const insetArea = signedPolygonArea(clean)
  const insetAbsArea = Math.abs(insetArea)
  if (
    insetAbsArea < EPSILON ||
    insetAbsArea > originalAbsArea + EPSILON ||
    Math.sign(insetArea) !== Math.sign(originalArea)
  ) {
    return undefined
  }

  return clean
}

function findBestPoint(candidates: ClipperPoint[], polygon: ClipperPoint[]) {
  let best: ClipperCircle | undefined

  for (const candidate of candidates) {
    if (!pointInConvexPolygon(candidate, polygon, EPSILON)) {
      continue
    }

    const radius = distanceToPolygonEdges(candidate, polygon)
    if (!best || radius > best.radius) {
      best = {
        center: candidate,
        radius,
      }
    }
  }

  return best
}

function pointInConvexPolygon(point: ClipperPoint, polygon: ClipperPoint[], tolerance: number) {
  const area = signedPolygonArea(polygon)
  const direction = area >= 0 ? 1 : -1

  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]
    const next = polygon[(index + 1) % polygon.length]
    const edge = subtract(next, current)
    const relative = subtract(point, current)
    const cross = crossProduct(edge, relative)

    if (direction * cross < -tolerance) {
      return false
    }
  }

  return true
}

function cleanPolygon(polygon: ClipperPoint[]) {
  const points: ClipperPoint[] = []

  for (const point of polygon) {
    const previous = points[points.length - 1]
    if (!previous || Math.hypot(point[0] - previous[0], point[1] - previous[1]) > EPSILON) {
      points.push([point[0], point[1]])
    }
  }

  const first = points[0]
  const last = points[points.length - 1]
  if (first && last && Math.hypot(first[0] - last[0], first[1] - last[1]) <= EPSILON) {
    points.pop()
  }

  return points
}

function signedPolygonArea(polygon: ClipperPoint[]) {
  let area = 0

  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index]
    const next = polygon[(index + 1) % polygon.length]
    area += current[0] * next[1] - next[0] * current[1]
  }

  return area / 2
}

function averagePoint(points: ClipperPoint[]): ClipperPoint {
  if (points.length === 0) {
    return [0, 0]
  }

  const total = points.reduce(
    (sum, point) => {
      sum[0] += point[0]
      sum[1] += point[1]
      return sum
    },
    [0, 0] as ClipperPoint,
  )

  return [total[0] / points.length, total[1] / points.length]
}

function polygonBounds(polygon: ClipperPoint[]) {
  return polygon.reduce(
    (bounds, point) => ({
      minX: Math.min(bounds.minX, point[0]),
      minY: Math.min(bounds.minY, point[1]),
      maxX: Math.max(bounds.maxX, point[0]),
      maxY: Math.max(bounds.maxY, point[1]),
    }),
    {
      minX: Infinity,
      minY: Infinity,
      maxX: -Infinity,
      maxY: -Infinity,
    },
  )
}

function lineIntersection(aStart: ClipperPoint, aEnd: ClipperPoint, bStart: ClipperPoint, bEnd: ClipperPoint) {
  const a = subtract(aEnd, aStart)
  const b = subtract(bEnd, bStart)
  const denominator = crossProduct(a, b)

  if (Math.abs(denominator) < EPSILON) {
    return undefined
  }

  const t = crossProduct(subtract(bStart, aStart), b) / denominator
  return [aStart[0] + a[0] * t, aStart[1] + a[1] * t] as ClipperPoint
}

function distanceToSegment(point: ClipperPoint, start: ClipperPoint, end: ClipperPoint) {
  const segment = subtract(end, start)
  const lengthSquared = dotProduct(segment, segment)
  if (lengthSquared < EPSILON) {
    return Math.hypot(point[0] - start[0], point[1] - start[1])
  }

  const projection = Math.max(0, Math.min(1, dotProduct(subtract(point, start), segment) / lengthSquared))
  const closest: ClipperPoint = [start[0] + segment[0] * projection, start[1] + segment[1] * projection]
  return Math.hypot(point[0] - closest[0], point[1] - closest[1])
}

function subtract(left: ClipperPoint, right: ClipperPoint): ClipperPoint {
  return [left[0] - right[0], left[1] - right[1]]
}

function dotProduct(left: ClipperPoint, right: ClipperPoint) {
  return left[0] * right[0] + left[1] * right[1]
}

function crossProduct(left: ClipperPoint, right: ClipperPoint) {
  return left[0] * right[1] - left[1] * right[0]
}
