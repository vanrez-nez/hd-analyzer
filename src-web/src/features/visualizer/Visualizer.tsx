import { useEffect, useRef } from "react"
import type { MouseEvent, MutableRefObject } from "react"
import { animate } from "motion"
import { ExternalLinkIcon } from "lucide-react"

import { openRepositoryHomepage } from "@/api"
import { Button } from "@/components/ui/button"
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import type { VisualizerLevelSnapshot } from "./types"
import { Honeycomb } from "./honeycomb"
import { LegendVisualizer } from "./legend-visualizer"
import { Voronoi } from "./voronoi"
import { visualizerColor } from "@/file-colors"
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
const SELECTION_HIGHLIGHT_FADE_MS = 140
const RESIZE_OVERLAY_FADE_MS = 250
const RESIZE_LABEL_FADE_MS = 140

type MotionControls = {
  stop: () => void
}

type VisualizerDrawState = {
  cellsVisible: boolean
  labelOpacity: number
  overlayOpacity: number
  placeholderVisible: boolean
}

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
  const drawStateRef = useRef<VisualizerDrawState>({
    cellsVisible: true,
    labelOpacity: 1,
    overlayOpacity: 0,
    placeholderVisible: false,
  })
  const observedWrapperSizeRef = useRef("")
  const resizeLabelAnimationRef = useRef<MotionControls | undefined>(undefined)
  const resizeOverlayAnimationRef = useRef<MotionControls | undefined>(undefined)
  const selectionAnimationRef = useRef<MotionControls | undefined>(undefined)
  const selectionProgressByIdRef = useRef<Map<string, number>>(new Map())

  useEffect(() => {
    const previousSelectedItemIds = new Set(selectedItemIdsRef.current)
    const nextSelectedItemIds = new Set(selectedItemIds)

    snapshotRef.current = snapshot
    colorSchemeRef.current = colorScheme
    selectedItemIdsRef.current = selectedItemIds

    selectionProgressByIdRef.current.forEach((_progress, itemId) => {
      if (!nextSelectedItemIds.has(itemId)) {
        selectionProgressByIdRef.current.delete(itemId)
      }
    })

    const newlySelectedItemIds: string[] = []
    selectedItemIds.forEach((itemId) => {
      if (!previousSelectedItemIds.has(itemId)) {
        selectionProgressByIdRef.current.set(itemId, 0)
        newlySelectedItemIds.push(itemId)
      }
    })

    if (selectionProgressByIdRef.current.size === 0) {
      selectionAnimationRef.current?.stop()
      selectionAnimationRef.current = undefined
    } else if (newlySelectedItemIds.length > 0) {
      startSelectionAnimation()
    }

    drawCurrentVisualizerFrame()
  }, [colorScheme, selectedItemIds, snapshot])

  const drawCurrentVisualizerFrame = () => {
    const drawState = drawStateRef.current
    if (!drawState.cellsVisible) {
      drawResizePlaceholder(canvasRef.current, wrapperRef.current, drawState.overlayOpacity)
    } else {
      drawVisualizer(
        canvasRef.current,
        wrapperRef.current,
        voronoiRef,
        snapshotRef.current,
        colorSchemeRef.current,
        selectedItemIdsRef.current,
        selectionProgressByIdRef.current,
        drawState,
      )
    }
  }

  const startSelectionAnimation = () => {
    selectionAnimationRef.current?.stop()
    const startProgressById = new Map(selectionProgressByIdRef.current)

    const animation = animate(0, 1, {
      duration: SELECTION_HIGHLIGHT_FADE_MS / 1000,
      ease: "easeOut",
      onUpdate: (progress) => {
        const selectedItemIdSet = new Set(selectedItemIdsRef.current)
        startProgressById.forEach((startProgress, itemId) => {
          if (!selectedItemIdSet.has(itemId)) {
            return
          }

          selectionProgressByIdRef.current.set(itemId, startProgress + (1 - startProgress) * progress)
        })
        drawCurrentVisualizerFrame()
      },
      onComplete: () => {
        if (selectionAnimationRef.current !== animation) {
          return
        }

        selectionAnimationRef.current = undefined
        startProgressById.forEach((_progress, itemId) => {
          selectionProgressByIdRef.current.delete(itemId)
        })
        drawCurrentVisualizerFrame()
      },
    })

    selectionAnimationRef.current = animation
  }

  const stopResizeAnimations = () => {
    resizeOverlayAnimationRef.current?.stop()
    resizeOverlayAnimationRef.current = undefined
    resizeLabelAnimationRef.current?.stop()
    resizeLabelAnimationRef.current = undefined
  }

  const startResizeLabelReveal = () => {
    resizeLabelAnimationRef.current?.stop()
    drawStateRef.current.labelOpacity = 0

    const animation = animate(0, 1, {
      duration: RESIZE_LABEL_FADE_MS / 1000,
      ease: "easeOut",
      onUpdate: (opacity) => {
        drawStateRef.current.labelOpacity = opacity
        drawCurrentVisualizerFrame()
      },
      onComplete: () => {
        if (resizeLabelAnimationRef.current !== animation) {
          return
        }

        resizeLabelAnimationRef.current = undefined
        drawStateRef.current.labelOpacity = 1
        drawCurrentVisualizerFrame()
      },
    })

    resizeLabelAnimationRef.current = animation
  }

  const startResizeReveal = () => {
    stopResizeAnimations()
    drawStateRef.current = {
      cellsVisible: true,
      labelOpacity: 0,
      overlayOpacity: 1,
      placeholderVisible: true,
    }
    drawCurrentVisualizerFrame()

    const animation = animate(1, 0, {
      duration: RESIZE_OVERLAY_FADE_MS / 1000,
      ease: "easeOut",
      onUpdate: (opacity) => {
        drawStateRef.current.overlayOpacity = opacity
        drawStateRef.current.placeholderVisible = opacity > 0.001
        drawCurrentVisualizerFrame()
      },
      onComplete: () => {
        if (resizeOverlayAnimationRef.current !== animation) {
          return
        }

        resizeOverlayAnimationRef.current = undefined
        drawStateRef.current.overlayOpacity = 0
        drawStateRef.current.placeholderVisible = false
        drawCurrentVisualizerFrame()
        startResizeLabelReveal()
      },
    })

    resizeOverlayAnimationRef.current = animation
  }

  useEffect(() => {
    let resizeTimeout: number | undefined
    const scheduleRedraw = () => {
      const wrapper = wrapperRef.current
      const sizeKey = wrapper ? `${wrapper.clientWidth}x${wrapper.clientHeight}` : ""
      if (sizeKey === observedWrapperSizeRef.current) {
        return
      }

      observedWrapperSizeRef.current = sizeKey
      stopResizeAnimations()
      drawStateRef.current = {
        cellsVisible: false,
        labelOpacity: 0,
        overlayOpacity: 1,
        placeholderVisible: true,
      }
      voronoiRef.current = undefined
      drawResizePlaceholder(canvasRef.current, wrapperRef.current, 1)
      window.clearTimeout(resizeTimeout)
      resizeTimeout = window.setTimeout(() => {
        startResizeReveal()
      }, VISUALIZER_RESIZE_DEBOUNCE_MS)
    }

    const wrapper = wrapperRef.current
    observedWrapperSizeRef.current = wrapper ? `${wrapper.clientWidth}x${wrapper.clientHeight}` : ""
    const resizeObserver = wrapper ? new ResizeObserver(scheduleRedraw) : undefined
    if (wrapper) {
      resizeObserver?.observe(wrapper)
    }

    window.addEventListener("resize", scheduleRedraw)
    return () => {
      window.clearTimeout(resizeTimeout)
      stopResizeAnimations()
      resizeObserver?.disconnect()
      window.removeEventListener("resize", scheduleRedraw)
    }
  }, [])

  useEffect(() => {
    return () => {
      stopResizeAnimations()
      selectionAnimationRef.current?.stop()
      selectionAnimationRef.current = undefined
      selectionProgressByIdRef.current.clear()
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
      <LegendVisualizer items={snapshot.items} />
      <div className="absolute bottom-3 right-3 flex select-none items-center gap-1.5 font-mono text-[0.6875rem] text-muted-foreground/55">
        <span>v{__APP_VERSION__}</span>
        <TooltipProvider delayDuration={250}>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                aria-label="Open repository homepage"
                className="size-6 rounded-sm text-muted-foreground/55 hover:text-muted-foreground"
                size="icon-sm"
                type="button"
                variant="ghost"
                onClick={() => void openRepositoryHomepage()}
              >
                <ExternalLinkIcon aria-hidden="true" data-icon="inline-start" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top">Repository homepage</TooltipContent>
          </Tooltip>
        </TooltipProvider>
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
  selectionProgressById?: ReadonlyMap<string, number>,
  drawState?: VisualizerDrawState,
) {
  const renderState = prepareVisualizer(canvas, wrapper, voronoiRef, snapshot, colorScheme)
  if (!renderState) {
    return false
  }

  const { context, height, layout, scaleX, scaleY, voronoi, width } = renderState
  context.setTransform(scaleX, 0, 0, scaleY, 0, 0)
  voronoi.drawBackground(context)
  // voronoi.drawDebugBase(context, layout)
  if (drawState?.cellsVisible ?? true) {
    context.save()
    honeycomb.draw(context, layout, {
      colorScheme,
      labelOpacity: drawState?.labelOpacity ?? 1,
      selectedItemIds,
      selectionProgressById,
    })
    context.restore()
  }
  voronoi.drawFrame(context, layout)
  if (drawState?.placeholderVisible && drawState.overlayOpacity > 0) {
    drawPlaceholderCircle(context, width, height, drawState.overlayOpacity)
  }
  return false
}

function drawResizePlaceholder(
  canvas: HTMLCanvasElement | null,
  wrapper: HTMLDivElement | null,
  opacity: number,
) {
  const renderState = prepareCanvas(canvas, wrapper)
  if (!renderState) {
    return
  }

  const { context, height, scaleX, scaleY, width } = renderState
  context.setTransform(scaleX, 0, 0, scaleY, 0, 0)
  context.clearRect(0, 0, width, height)
  drawPlaceholderCircle(context, width, height, opacity)
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

  const canvasState = prepareCanvas(canvas, wrapper)
  if (!canvasState) {
    return undefined
  }

  const { context, paddingLeft, paddingTop, scaleX, scaleY, width, height } = canvasState

  if (canvasState.resized) {
    voronoiRef.current = new Voronoi(width, height, snapshot, colorScheme)
  }

  voronoiRef.current ??= new Voronoi(width, height, snapshot, colorScheme)
  const layout = voronoiRef.current.getLayout(snapshot, colorScheme)
  return {
    context,
    height,
    layout,
    paddingLeft,
    paddingTop,
    scaleX,
    scaleY,
    voronoi: voronoiRef.current,
    width,
  }
}

function prepareCanvas(canvas: HTMLCanvasElement | null, wrapper: HTMLDivElement | null) {
  if (!canvas || !wrapper) {
    return undefined
  }

  const { paddingLeft, paddingTop, width, height } = getCanvasContentMetrics(canvas, wrapper)
  const devicePixelRatio = window.devicePixelRatio || 1
  const backingWidth = Math.max(1, Math.round(width * devicePixelRatio))
  const backingHeight = Math.max(1, Math.round(height * devicePixelRatio))
  const scaleX = backingWidth / width
  const scaleY = backingHeight / height
  const resized = canvas.width !== backingWidth || canvas.height !== backingHeight

  if (resized) {
    canvas.width = backingWidth
    canvas.height = backingHeight
  }

  const context = canvas.getContext("2d")
  if (!context) {
    return undefined
  }

  return {
    context,
    height,
    paddingLeft,
    paddingTop,
    resized,
    scaleX,
    scaleY,
    width,
  }
}

function drawPlaceholderCircle(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  opacity: number,
) {
  const radius = Math.max(1, Math.min(width, height) / 2)
  context.save()
  context.beginPath()
  context.arc(width / 2, height / 2, radius, 0, Math.PI * 2)
  context.fillStyle = visualizerColor("placeholderFill")
  context.globalAlpha = clamp(opacity, 0, 1)
  context.fill()
  context.restore()
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

function clamp(value: number, min: number, max: number) {
  if (!Number.isFinite(value)) {
    return min
  }

  return Math.min(max, Math.max(min, value))
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
