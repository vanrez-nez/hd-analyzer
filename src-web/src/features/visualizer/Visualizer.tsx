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
const VISUALIZER_RESIZE_DEBOUNCE_MS = 100

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

  useEffect(() => {
    let resizeTimeout: number | undefined
    const scheduleRedraw = () => {
      window.clearTimeout(resizeTimeout)
      resizeTimeout = window.setTimeout(() => {
        drawVisualizer(
          canvasRef.current,
          wrapperRef.current,
          voronoiRef,
          snapshotRef.current,
          colorSchemeRef.current,
        )
      }, VISUALIZER_RESIZE_DEBOUNCE_MS)
    }

    const wrapper = wrapperRef.current
    const resizeObserver = wrapper ? new ResizeObserver(scheduleRedraw) : undefined
    if (wrapper) {
      resizeObserver?.observe(wrapper)
    }

    window.addEventListener("resize", scheduleRedraw)
    return () => {
      window.clearTimeout(resizeTimeout)
      resizeObserver?.disconnect()
      window.removeEventListener("resize", scheduleRedraw)
    }
  }, [])

  return (
    <div ref={wrapperRef} className="relative h-full w-full overflow-hidden">
      <canvas
        ref={canvasRef}
        className="absolute left-1/2 top-1/2 block -translate-x-1/2 -translate-y-1/2"
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
  canvas.style.width = `${width}px`
  canvas.style.height = `${height}px`

  if (canvas.width !== backingWidth || canvas.height !== backingHeight) {
    canvas.width = backingWidth
    canvas.height = backingHeight
    voronoiRef.current = new Voronoi(width, height, snapshot, colorScheme)
  }

  const context = canvas.getContext("2d")
  if (!context) {
    return false
  }

  context.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0)
  voronoiRef.current ??= new Voronoi(width, height, snapshot, colorScheme)
  const layout = voronoiRef.current.getLayout(snapshot, colorScheme)
  voronoiRef.current.drawBackground(context)
  // voronoiRef.current.drawDebugBase(context, layout)
  honeycomb.draw(context, layout, { colorScheme })
  voronoiRef.current.drawFrame(context, layout)
  return false
}
