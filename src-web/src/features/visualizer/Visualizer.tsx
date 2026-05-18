import { useEffect, useMemo, useRef, useState } from "react"
import type { MutableRefObject } from "react"

import { RafLoop } from "@/lib/raf-loop"
import type { VisualizerLevelSnapshot } from "./types"
import { Voronoi } from "./voronoi"

type VisualizerProps = {
  snapshot: VisualizerLevelSnapshot
}

export function Visualizer({ snapshot }: VisualizerProps) {
  const [fps, setFps] = useState(0)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const frameCountRef = useRef(0)
  const needsDrawRef = useRef(true)
  const sampleElapsedRef = useRef(0)
  const wrapperRef = useRef<HTMLDivElement>(null)
  const voronoiRef = useRef<Voronoi | undefined>(undefined)
  const snapshotRef = useRef(snapshot)
  const snapshotSignature = useMemo(() => createSnapshotSignature(snapshot), [snapshot])

  useEffect(() => {
    snapshotRef.current = snapshot
    voronoiRef.current?.setSnapshot(snapshot)
    needsDrawRef.current = true
  }, [snapshot, snapshotSignature])

  useEffect(() => {
    const loop = new RafLoop((deltaSeconds) => {
      frameCountRef.current += 1
      sampleElapsedRef.current += deltaSeconds

      if (sampleElapsedRef.current >= 0.5) {
        setFps(Math.round(frameCountRef.current / sampleElapsedRef.current))
        frameCountRef.current = 0
        sampleElapsedRef.current = 0
      }

      if (needsDrawRef.current) {
        needsDrawRef.current = drawVisualizer(
          canvasRef.current,
          wrapperRef.current,
          voronoiRef,
          snapshotRef.current,
        )
      }
    })

    loop.start()
    return () => loop.stop()
  }, [])

  useEffect(() => {
    const wrapper = wrapperRef.current
    if (!wrapper) {
      return
    }

    const observer = new ResizeObserver(() => {
      needsDrawRef.current = true
    })
    observer.observe(wrapper)
    needsDrawRef.current = true

    return () => observer.disconnect()
  }, [])

  return (
    <div ref={wrapperRef} className="relative h-full w-full overflow-hidden">
      <canvas ref={canvasRef} className="block h-full w-full" aria-label="Visualizer canvas" />
      <div className="pointer-events-none absolute right-2 top-2 text-xs tabular-nums text-muted-foreground">
        {fps} FPS
      </div>
    </div>
  )
}

function drawVisualizer(
  canvas: HTMLCanvasElement | null,
  wrapper: HTMLDivElement | null,
  voronoiRef: MutableRefObject<Voronoi | undefined>,
  snapshot: VisualizerLevelSnapshot,
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
  return voronoiRef.current.draw(context)
}

function createSnapshotSignature(snapshot: VisualizerLevelSnapshot) {
  return `${snapshot.path ?? "volumes"}:${snapshot.parentPath ?? ""}:${snapshot.generation}`
}
