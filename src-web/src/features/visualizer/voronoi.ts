import { Delaunay } from "d3-delaunay"

type VoronoiPoint = {
  x: number
  y: number
}

type VoronoiOptions = {
  pointCount?: number
  seed?: number
}

const DEFAULT_POINT_COUNT = 96
const DEFAULT_SEED = 0x5eed

export class Voronoi {
  private readonly colors: string[]
  private readonly height: number
  private readonly points: VoronoiPoint[]
  private readonly width: number

  constructor(width: number, height: number, options: VoronoiOptions = {}) {
    this.width = Math.max(1, width)
    this.height = Math.max(1, height)
    this.points = createPoints(this.width, this.height, options.pointCount ?? DEFAULT_POINT_COUNT, options.seed ?? DEFAULT_SEED)
    this.colors = createPalette()
  }

  draw(context: CanvasRenderingContext2D) {
    const delaunay = Delaunay.from(
      this.points,
      (point: VoronoiPoint) => point.x,
      (point: VoronoiPoint) => point.y,
    )
    const voronoi = delaunay.voronoi([0, 0, this.width, this.height])

    context.clearRect(0, 0, this.width, this.height)
    context.fillStyle = "hsl(0 0% 0% / 0.02)"
    context.fillRect(0, 0, this.width, this.height)

    this.points.forEach((point, index) => {
      context.beginPath()
      voronoi.renderCell(index, context)
      context.fillStyle = this.colors[index % this.colors.length]
      context.fill()
      context.strokeStyle = "hsl(0 0% 100% / 0.12)"
      context.lineWidth = 1
      context.stroke()

      context.beginPath()
      context.arc(point.x, point.y, 1.4, 0, Math.PI * 2)
      context.fillStyle = "hsl(0 0% 100% / 0.5)"
      context.fill()
    })
  }
}

function createPoints(width: number, height: number, pointCount: number, seed: number) {
  const random = createRandom(seed)
  return Array.from({ length: pointCount }, () => ({
    x: random() * width,
    y: random() * height,
  }))
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

function createRandom(seed: number) {
  let state = seed >>> 0
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0
    return state / 0x100000000
  }
}
