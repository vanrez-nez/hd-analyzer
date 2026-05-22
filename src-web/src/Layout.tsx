import { useCallback, useEffect, useRef, useState } from "react"
import type { PanelImperativeHandle } from "react-resizable-panels"
import { animate } from "motion"

import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable"
import { FsExplorer } from "@/features/fs-explorer/FsExplorer"
import { Visualizer } from "@/features/visualizer/Visualizer"
import type { VisualizerLevelSnapshot } from "@/features/visualizer/types"
import type { ColorScheme } from "@/lib/use-system-color-scheme"
import { cn } from "@/lib/utils"

type SplitLayout = {
  explorer: number
  visualizer: number
}

const DEFAULT_SPLIT_LAYOUT: SplitLayout = {
  explorer: 30,
  visualizer: 70,
}
const SPLIT_ANIMATION_MS = 180
const VISUALIZER_COLLAPSED_SIZE_THRESHOLD = 0.5
const VISUALIZER_TOP_OFFSET_CLASS = "pt-11"

type MotionControls = {
  stop: () => void
}

type LayoutProps = {
  colorScheme: ColorScheme
  permissionChecking: boolean
  onRequestPermissions: () => void
}

export function Layout({ colorScheme, permissionChecking, onRequestPermissions }: LayoutProps) {
  const [selectedItemIds, setSelectedItemIds] = useState<string[]>([])
  const [explorerSelectionAnchorId, setExplorerSelectionAnchorId] = useState<string | null>(null)
  const [splitOpen, setSplitOpen] = useState(false)
  const [splitAnimating, setSplitAnimating] = useState(false)
  const [splitLayout, setSplitLayout] = useState<SplitLayout>(DEFAULT_SPLIT_LAYOUT)
  const [visualizerSnapshot, setVisualizerSnapshot] = useState<VisualizerLevelSnapshot>({
    path: null,
    parentPath: null,
    items: [],
    generation: "volumes:",
  })
  const splitAnimationRef = useRef<MotionControls | undefined>(undefined)
  const explorerPanelRef = useRef<PanelImperativeHandle | null>(null)
  const hasMountedRef = useRef(false)
  const ignoreLayoutChangeRef = useRef(false)
  const skipNextCloseAnimationRef = useRef(false)
  const splitLayoutRef = useRef<SplitLayout>(DEFAULT_SPLIT_LAYOUT)
  const visualizerPanelRef = useRef<PanelImperativeHandle | null>(null)
  const resizeDisabled = !splitOpen && !splitAnimating

  const handleVisualizerSnapshotChange = useCallback((snapshot: VisualizerLevelSnapshot) => {
    setVisualizerSnapshot(snapshot)
  }, [])

  const handleVisualizerToggle = useCallback(() => {
    setSplitAnimating(true)
    setSplitOpen((open) => !open)
  }, [])

  useEffect(() => {
    if (!hasMountedRef.current) {
      hasMountedRef.current = true
      visualizerPanelRef.current?.collapse()
      explorerPanelRef.current?.resize("100%")
      return
    }

    const explorerPanel = explorerPanelRef.current
    const visualizerPanel = visualizerPanelRef.current
    if (!explorerPanel || !visualizerPanel) {
      return
    }

    splitAnimationRef.current?.stop()
    splitAnimationRef.current = undefined

    if (!splitOpen && skipNextCloseAnimationRef.current) {
      skipNextCloseAnimationRef.current = false
      explorerPanel.resize("100%")
      visualizerPanel.collapse()
      ignoreLayoutChangeRef.current = false
      setSplitAnimating(false)
      return
    }

    setSplitAnimating(true)
    ignoreLayoutChangeRef.current = true

    const targetLayout = splitLayoutRef.current
    const startExplorer = getPanelPercentage(explorerPanel, splitOpen ? 100 : targetLayout.explorer)
    const startVisualizer = getPanelPercentage(visualizerPanel, splitOpen ? 0 : targetLayout.visualizer)
    const targetExplorer = splitOpen ? targetLayout.explorer : 100
    const targetVisualizer = splitOpen ? targetLayout.visualizer : 0

    const animation = animate(0, 1, {
      duration: SPLIT_ANIMATION_MS / 1000,
      ease: "easeOut",
      onUpdate: (progress) => {
        const explorerSize = interpolateNumber(startExplorer, targetExplorer, progress)
        const visualizerSize = interpolateNumber(startVisualizer, targetVisualizer, progress)

        explorerPanel.resize(`${explorerSize}%`)
        visualizerPanel.resize(`${visualizerSize}%`)
      },
      onComplete: () => {
        if (splitAnimationRef.current !== animation) {
          return
        }

        splitAnimationRef.current = undefined
        explorerPanel.resize(`${targetExplorer}%`)
        if (splitOpen) {
          visualizerPanel.resize(`${targetVisualizer}%`)
        } else {
          visualizerPanel.collapse()
        }

        ignoreLayoutChangeRef.current = false
        setSplitAnimating(false)
      },
    })

    splitAnimationRef.current = animation

    return () => {
      if (splitAnimationRef.current === animation) {
        animation.stop()
        splitAnimationRef.current = undefined
      }
    }
  }, [splitOpen])

  useEffect(() => {
    return () => {
      splitAnimationRef.current?.stop()
      splitAnimationRef.current = undefined
    }
  }, [])

  return (
    <ResizablePanelGroup
      direction="horizontal"
      className={cn("h-full min-h-0 flex-1", (splitOpen || splitAnimating) && "gap-2")}
      defaultLayout={splitLayout}
      disabled={resizeDisabled}
      onLayoutChanged={(layout) => {
        if (
          ignoreLayoutChangeRef.current ||
          !splitOpen ||
          layout.visualizer === undefined
        ) {
          return
        }

        if (layout.visualizer <= VISUALIZER_COLLAPSED_SIZE_THRESHOLD) {
          skipNextCloseAnimationRef.current = true
          setSplitOpen(false)
          return
        }

        const nextLayout = {
          explorer: layout.explorer ?? DEFAULT_SPLIT_LAYOUT.explorer,
          visualizer: layout.visualizer,
        }

        splitLayoutRef.current = nextLayout
        setSplitLayout(nextLayout)
      }}
    >
      <ResizablePanel
        id="explorer"
        defaultSize="100%"
        disabled={resizeDisabled}
        minSize="20%"
        panelRef={explorerPanelRef}
        className="flex min-h-0 min-w-0 overflow-hidden"
      >
        <FsExplorer
          explorerSelectionAnchorId={explorerSelectionAnchorId}
          permissionChecking={permissionChecking}
          selectedItemIds={selectedItemIds}
          visualizerOpen={splitOpen}
          onExplorerSelectionAnchorChange={setExplorerSelectionAnchorId}
          onRequestPermissions={onRequestPermissions}
          onSelectionChange={setSelectedItemIds}
          onVisualizerToggle={handleVisualizerToggle}
          onVisualizerSnapshotChange={handleVisualizerSnapshotChange}
        />
      </ResizablePanel>
      {splitOpen && !splitAnimating ? (
        <ResizableHandle withHandle className="mx-2 w-[6px] bg-transparent after:hidden" />
      ) : null}
      <ResizablePanel
        id="visualizer"
        collapsible
        collapsedSize="0%"
        defaultSize="0%"
        disabled={resizeDisabled}
        minSize={splitAnimating || !splitOpen ? "0%" : "25%"}
        panelRef={visualizerPanelRef}
        className="min-h-0 min-w-0 overflow-hidden"
      >
        <div className={cn("flex h-full min-h-0 flex-col", VISUALIZER_TOP_OFFSET_CLASS)}>
          <div className="min-h-0 flex-1">
            <Visualizer
              colorScheme={colorScheme}
              selectedItemIds={selectedItemIds}
              snapshot={visualizerSnapshot}
              onExplorerSelectionAnchorChange={setExplorerSelectionAnchorId}
              onSelectionChange={setSelectedItemIds}
            />
          </div>
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
  )
}

function getPanelPercentage(panel: PanelImperativeHandle, fallback: number) {
  const size = panel.getSize().asPercentage
  return Number.isFinite(size) ? size : fallback
}

function interpolateNumber(from: number, to: number, amount: number) {
  return from + (to - from) * amount
}
