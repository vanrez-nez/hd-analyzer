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
        className="absolute block box-border p-16"
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

  const { context, layout, scaleX, scaleY, voronoi } = renderState
  context.setTransform(scaleX, 0, 0, scaleY, 0, 0)
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

  const { paddingLeft, paddingTop, width, height } = getCanvasContentMetrics(canvas, wrapper)
  const devicePixelRatio = window.devicePixelRatio || 1
  const backingWidth = Math.max(1, Math.round(width * devicePixelRatio))
  const backingHeight = Math.max(1, Math.round(height * devicePixelRatio))
  const scaleX = backingWidth / width
  const scaleY = backingHeight / height

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
    layout,
    paddingLeft,
    paddingTop,
    scaleX,
    scaleY,
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

  const point = canvasPointFromEvent(event, canvas, renderState.paddingLeft, renderState.paddingTop)
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

function getCanvasContentMetrics(canvas: HTMLCanvasElement, wrapper: HTMLDivElement) {
  const canvasSize = Math.max(1, Math.floor(Math.min(wrapper.clientWidth, wrapper.clientHeight)))
  const canvasLeft = Math.max(0, Math.floor((wrapper.clientWidth - canvasSize) / 2))
  const canvasTop = Math.max(0, Math.floor((wrapper.clientHeight - canvasSize) / 2))
  canvas.style.left = `${canvasLeft}px`
  canvas.style.top = `${canvasTop}px`
  canvas.style.width = `${canvasSize}px`
  canvas.style.height = `${canvasSize}px`

  const style = window.getComputedStyle(canvas)
  const paddingLeft = parseCssPixels(style.paddingLeft)
  const paddingRight = parseCssPixels(style.paddingRight)
  const paddingTop = parseCssPixels(style.paddingTop)
  const paddingBottom = parseCssPixels(style.paddingBottom)
  const width = Math.max(1, canvasSize - Math.ceil(paddingLeft + paddingRight))
  const height = Math.max(1, canvasSize - Math.ceil(paddingTop + paddingBottom))

  return {
    paddingLeft,
    paddingTop,
    width,
    height,
  }
}

function parseCssPixels(value: string) {
  const parsed = Number.parseFloat(value)
  return Number.isFinite(parsed) ? parsed : 0
}

function canvasPointFromEvent(
  event: MouseEvent<HTMLCanvasElement>,
  canvas: HTMLCanvasElement,
  paddingLeft: number,
  paddingTop: number,
) {
  const rect = canvas.getBoundingClientRect()
  return [event.clientX - rect.left - paddingLeft, event.clientY - rect.top - paddingTop] as [number, number]
}
