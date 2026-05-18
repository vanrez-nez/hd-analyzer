import { voronoiMapInitialPositionPie, voronoiMapSimulation } from "d3-voronoi-map"

import type { VisualizerCellInput, VisualizerLevelSnapshot } from "./types"

type WeightedCell = VisualizerCellInput & {
  weight: number
}

type VoronoiPoint = [number, number]

type VoronoiPolygon = VoronoiPoint[] & {
  site?: {
    originalObject?: {
      data?: {
        originalData?: WeightedCell
      }
    }
  }
}

type VoronoiState = {
  ended: boolean
  polygons: VoronoiPolygon[]
}

type VoronoiSimulation = {
  tick: () => void
  state: () => VoronoiState
  stop: () => VoronoiSimulation
}

type RenderCell = {
  item: WeightedCell
  polygon: VoronoiPoint[]
}

type VoronoiLayout = {
  generation: string
  path: string | null
  cells: RenderCell[]
}

type CircleBounds = {
  centerX: number
  centerY: number
  radius: number
}

const DEFAULT_SEED = 0x5eed
const MAX_ITERATIONS = 80
const CONVERGENCE_RATIO = 0.002
const MIN_WEIGHT_RATIO = 1e-9
const CIRCLE_SEGMENTS = 96

export class Voronoi {
  private readonly circle: CircleBounds
  private readonly colors: string[]
  private currentLayout: VoronoiLayout
  private readonly fullBoundary: VoronoiPoint[]
  private readonly height: number
  private readonly width: number

  constructor(width: number, height: number, snapshot: VisualizerLevelSnapshot) {
    this.width = Math.max(1, width)
    this.height = Math.max(1, height)
    this.circle = createCircleBounds(this.width, this.height)
    this.fullBoundary = createCircleBoundary(this.circle)
    this.colors = createPalette()
    this.currentLayout = createLayout(snapshot, this.fullBoundary) ?? createEmptyLayout(snapshot)
  }

  setSnapshot(snapshot: VisualizerLevelSnapshot) {
    if (snapshot.path === this.currentLayout.path && snapshot.generation === this.currentLayout.generation) {
      return
    }

    const nextLayout = createLayout(snapshot, this.fullBoundary)
    if (nextLayout) {
      this.currentLayout = nextLayout
    }
  }

  draw(context: CanvasRenderingContext2D) {
    context.clearRect(0, 0, this.width, this.height)
    drawCircleBackground(context, this.circle)
    drawContainedCells(context, this.circle, this.currentLayout.cells, this.colors)
    drawCircleFrame(context, this.circle)
    return false
  }
}

function createLayout(snapshot: VisualizerLevelSnapshot, boundary: VoronoiPoint[]): VoronoiLayout | undefined {
  const cells = createCells(snapshot, boundary, createWeightedCells(snapshot.items))
  if (!cells) {
    return undefined
  }

  return {
    generation: snapshot.generation,
    path: snapshot.path,
    cells,
  }
}

function createEmptyLayout(snapshot: VisualizerLevelSnapshot): VoronoiLayout {
  return {
    generation: snapshot.generation,
    path: snapshot.path,
    cells: [],
  }
}

function createCells(
  snapshot: VisualizerLevelSnapshot,
  boundary: VoronoiPoint[],
  items: WeightedCell[],
): RenderCell[] | undefined {
  if (items.length === 0) {
    return []
  }

  if (items.length === 1) {
    return [createRenderCell(items[0], boundary)]
  }

  try {
    const simulation = voronoiMapSimulation(items)
      .clip(boundary)
      .weight((item: WeightedCell) => item.weight)
      .minWeightRatio(MIN_WEIGHT_RATIO)
      .convergenceRatio(CONVERGENCE_RATIO)
      .maxIterationCount(MAX_ITERATIONS)
      .initialPosition(voronoiMapInitialPositionPie())
      .prng(createRandom(hashLayout(snapshot, items)))
      .stop() as VoronoiSimulation

    let state = simulation.state()
    while (!state.ended) {
      simulation.tick()
      state = simulation.state()
    }

    return state.polygons
      .map((polygon) => {
        const item = polygon.site?.originalObject?.data?.originalData
        return item ? createRenderCell(item, polygon) : undefined
      })
      .filter((cell): cell is RenderCell => Boolean(cell))
  } catch {
    return undefined
  }
}

function createRenderCell(item: WeightedCell, polygon: VoronoiPoint[]): RenderCell {
  return {
    item,
    polygon: polygon.map(([x, y]): VoronoiPoint => [x, y]),
  }
}

function createWeightedCells(items: VisualizerCellInput[]) {
  return items
    .filter((item) => Number.isFinite(item.size) && item.size > 0)
    .map((item) => ({ ...item, weight: item.size }))
}

function createPalette() {
  return [
    "hsl(196 42% 32% / 0.56)",
    "hsl(156 28% 36% / 0.52)",
    "hsl(34 40% 43% / 0.50)",
    "hsl(0 0% 28% / 0.48)",
    "hsl(84 24% 34% / 0.48)",
    "hsl(12 35% 38% / 0.46)",
  ]
}

function createCircleBounds(width: number, height: number): CircleBounds {
  return {
    centerX: width / 2,
    centerY: height / 2,
    radius: Math.max(1, Math.min(width, height) / 2),
  }
}

function createCircleBoundary(circle: CircleBounds) {
  return Array.from({ length: CIRCLE_SEGMENTS }, (_, index): VoronoiPoint => {
    const angle = -Math.PI / 2 - (index / CIRCLE_SEGMENTS) * Math.PI * 2
    return [
      circle.centerX + Math.cos(angle) * circle.radius,
      circle.centerY + Math.sin(angle) * circle.radius,
    ]
  })
}

function drawCircleBackground(context: CanvasRenderingContext2D, circle: CircleBounds) {
  context.save()
  drawCirclePath(context, circle)
  context.fillStyle = "hsl(0 0% 0% / 0.02)"
  context.fill()
  context.restore()
}

function drawCircleFrame(context: CanvasRenderingContext2D, circle: CircleBounds) {
  context.save()
  drawCirclePath(context, circle)
  context.strokeStyle = "hsl(0 0% 100% / 0.16)"
  context.lineWidth = 1
  context.stroke()
  context.restore()
}

function drawContainedCells(
  context: CanvasRenderingContext2D,
  circle: CircleBounds,
  cells: RenderCell[],
  colors: string[],
) {
  context.save()
  drawCirclePath(context, circle)
  context.clip()
  cells.forEach((cell) => drawPolygon(context, cell.polygon, colorForItem(cell.item, colors)))
  context.restore()
}

function drawPolygon(context: CanvasRenderingContext2D, polygon: VoronoiPoint[], fillStyle: string) {
  if (polygon.length === 0) {
    return
  }

  context.beginPath()
  polygon.forEach(([x, y], index) => {
    if (index === 0) {
      context.moveTo(x, y)
      return
    }

    context.lineTo(x, y)
  })
  context.closePath()
  context.fillStyle = fillStyle
  context.fill()
  context.strokeStyle = "hsl(0 0% 100% / 0.12)"
  context.lineWidth = 1
  context.stroke()
}

function drawCirclePath(context: CanvasRenderingContext2D, circle: CircleBounds) {
  context.beginPath()
  context.arc(circle.centerX, circle.centerY, circle.radius, 0, Math.PI * 2)
}

function colorForItem(item: WeightedCell, colors: string[]) {
  return colors[hashString(item.id) % colors.length]
}

function hashLayout(snapshot: VisualizerLevelSnapshot, items: WeightedCell[]) {
  return items.reduce(
    (hash, item) => hash ^ hashString(`${item.id}:${item.path}`),
    hashString(snapshot.path ?? "volumes") ^ DEFAULT_SEED,
  ) >>> 0
}

function hashString(value: string) {
  let hash = 2166136261
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }

  return hash >>> 0
}

function createRandom(seed: number) {
  let state = seed >>> 0
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0
    return state / 0x100000000
  }
}
