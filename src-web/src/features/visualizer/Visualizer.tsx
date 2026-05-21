import { useEffect, useRef } from "react"
import type { MouseEvent, MutableRefObject } from "react"

import type { VisualizerLevelSnapshot } from "./types"
import { Honeycomb } from "./honeycomb"
import { Voronoi } from "./voronoi"
import { isToggleSelectionInput, replaceSelection, toggleSelection } from "@/lib/selection"
import type { ColorScheme } from "@/lib/use-system-color-scheme"
import type { VisualizerLayoutCell } from "./voronoi"

type VisualizerProps = {
  colorScheme: ColorScheme
  selectedItemIds: string[]
  snapshot: VisualizerLevelSnapshot
  onExplorerSelectionAnchorChange: (itemId: string | null) => void
  onSelectionChange: (itemIds: string[]) => void
}

const honeycomb = new Honeycomb()
const VISUALIZER_RESIZE_DEBOUNCE_MS = 100

export function Visualizer({
  colorScheme,
  selectedItemIds,
  snapshot,
  onExplorerSelectionAnchorChange,
  onSelectionChange,
}: VisualizerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const wrapperRef = useRef<HTMLDivElement>(null)
  const voronoiRef = useRef<Voronoi | undefined>(undefined)
  const snapshotRef = useRef(snapshot)
  const colorSchemeRef = useRef(colorScheme)
  const selectedItemIdsRef = useRef(selectedItemIds)

  useEffect(() => {
    snapshotRef.current = snapshot
    colorSchemeRef.current = colorScheme
    selectedItemIdsRef.current = selectedItemIds
    drawVisualizer(canvasRef.current, wrapperRef.current, voronoiRef, snapshot, colorScheme, selectedItemIds)
  }, [colorScheme, selectedItemIds, snapshot])

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
          selectedItemIdsRef.current,
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
        className="absolute left-1/2 top-1/2 block -translate-x-1/2 -translate-y-1/2 p-16"
        aria-label="Visualizer canvas"
        onClick={(event) => {
          const cell = selectedCellAtPoint(
            event,
            canvasRef.current,
            wrapperRef.current,
            voronoiRef,
            snapshotRef.current,
            colorSchemeRef.current,
          )
          const nextSelection = selectionForCellClick(
            selectedItemIdsRef.current,
            cell,
            snapshotRef.current,
            event,
          )
          onSelectionChange(nextSelection)
          onExplorerSelectionAnchorChange(nextSelection[0] ?? null)
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
  selectedItemIds: string[],
) {
  const renderState = prepareVisualizer(canvas, wrapper, voronoiRef, snapshot, colorScheme)
  if (!renderState) {
    return false
  }

  const { context, devicePixelRatio, layout, voronoi } = renderState
  context.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0)
  voronoi.drawBackground(context)
  // voronoi.drawDebugBase(context, layout)
  honeycomb.draw(context, layout, { colorScheme, selectedItemIds })
  voronoi.drawFrame(context, layout)
  return false
}

function prepareVisualizer(
  canvas: HTMLCanvasElement | null,
  wrapper: HTMLDivElement | null,
  voronoiRef: MutableRefObject<Voronoi | undefined>,
  snapshot: VisualizerLevelSnapshot,
  colorScheme: ColorScheme,
) {
  if (!canvas || !wrapper) {
    return undefined
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
    return undefined
  }

  voronoiRef.current ??= new Voronoi(width, height, snapshot, colorScheme)
  const layout = voronoiRef.current.getLayout(snapshot, colorScheme)
  return {
    context,
    devicePixelRatio,
    layout,
    voronoi: voronoiRef.current,
  }
}

function selectedCellAtPoint(
  event: MouseEvent<HTMLCanvasElement>,
  canvas: HTMLCanvasElement | null,
  wrapper: HTMLDivElement | null,
  voronoiRef: MutableRefObject<Voronoi | undefined>,
  snapshot: VisualizerLevelSnapshot,
  colorScheme: ColorScheme,
) {
  const renderState = prepareVisualizer(canvas, wrapper, voronoiRef, snapshot, colorScheme)
  if (!renderState || !canvas) {
    return undefined
  }

  const point = canvasPointFromEvent(event, canvas)
  return honeycomb.hitTest(renderState.layout, point, { colorScheme })
}

function selectionForCellClick(
  currentSelection: string[],
  cell: VisualizerLayoutCell | undefined,
  snapshot: VisualizerLevelSnapshot,
  event: MouseEvent<HTMLCanvasElement>,
) {
  if (!cell) {
    return []
  }

  const memberIds = cell.memberIds.length > 0 ? cell.memberIds : [cell.selectionId]
  if (snapshot.path === null) {
    return memberIds[0] ? replaceSelection([memberIds[0]]) : []
  }

  if (isToggleSelectionInput(event)) {
    return toggleSelection(currentSelection, memberIds)
  }

  return replaceSelection(memberIds)
}

function canvasPointFromEvent(event: MouseEvent<HTMLCanvasElement>, canvas: HTMLCanvasElement) {
  const rect = canvas.getBoundingClientRect()
  return [event.clientX - rect.left, event.clientY - rect.top] as [number, number]
}
