import { useEffect, useRef } from "react"
import type { MutableRefObject } from "react"

import type { VisualizerLevelSnapshot } from "./types"
import { Honeycomb } from "./honeycomb"
import { Voronoi } from "./voronoi"
import type { ColorScheme } from "@/lib/use-system-color-scheme"

type VisualizerProps = {
  colorScheme: ColorScheme
  snapshot: VisualizerLevelSnapshot
}

const honeycomb = new Honeycomb()

export function Visualizer({ colorScheme, snapshot }: VisualizerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const wrapperRef = useRef<HTMLDivElement>(null)
  const voronoiRef = useRef<Voronoi | undefined>(undefined)
  const snapshotRef = useRef(snapshot)
  const colorSchemeRef = useRef(colorScheme)

  useEffect(() => {
    snapshotRef.current = snapshot
    colorSchemeRef.current = colorScheme
    drawVisualizer(canvasRef.current, wrapperRef.current, voronoiRef, snapshot, colorScheme)
  }, [colorScheme, snapshot])

  return (
    <div ref={wrapperRef} className="relative h-full w-full overflow-hidden">
      <canvas
        ref={canvasRef}
        className="block h-full w-full"
        aria-label="Visualizer canvas"
        onClick={() => {
          drawVisualizer(canvasRef.current, wrapperRef.current, voronoiRef, snapshotRef.current, colorSchemeRef.current)
        }}
      />
      <div className="pointer-events-none absolute right-2 top-2 text-xs tabular-nums text-muted-foreground">
        0 FPS
      </div>
    </div>
  )
}

function drawVisualizer(
  canvas: HTMLCanvasElement | null,
  wrapper: HTMLDivElement | null,
  voronoiRef: MutableRefObject<Voronoi | undefined>,
  snapshot: VisualizerLevelSnapshot,
  colorScheme: ColorScheme,
) {
  if (!canvas || !wrapper) {
    return false
  }

  const width = Math.max(1, Math.floor(wrapper.clientWidth))
  const height = Math.max(1, Math.floor(wrapper.clientHeight))
  const devicePixelRatio = window.devicePixelRatio || 1
  const backingWidth = Math.floor(width * devicePixelRatio)
  const backingHeight = Math.floor(height * devicePixelRatio)

  if (canvas.width !== backingWidth || canvas.height !== backingHeight) {
    canvas.width = backingWidth
    canvas.height = backingHeight
    voronoiRef.current = new Voronoi(width, height, snapshot)
  }

  const context = canvas.getContext("2d")
  if (!context) {
    return false
  }

  context.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0)
  voronoiRef.current ??= new Voronoi(width, height, snapshot)
  const layout = voronoiRef.current.getLayout(snapshot)
  voronoiRef.current.drawBackground(context)
  // voronoiRef.current.drawDebugBase(context, layout)
  honeycomb.draw(context, layout, { colorScheme })
  voronoiRef.current.drawFrame(context, layout)
  return false
}
