import { useCallback, useEffect, useRef, useState } from "react"
import type { PanelImperativeHandle } from "react-resizable-panels"

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

type LayoutProps = {
  colorScheme: ColorScheme
}

export function Layout({ colorScheme }: LayoutProps) {
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null)
  const [splitOpen, setSplitOpen] = useState(false)
  const [splitAnimating, setSplitAnimating] = useState(false)
  const [splitLayout, setSplitLayout] = useState<SplitLayout>(DEFAULT_SPLIT_LAYOUT)
  const [visualizerSnapshot, setVisualizerSnapshot] = useState<VisualizerLevelSnapshot>({
    path: null,
    parentPath: null,
    items: [],
    generation: "volumes:",
  })
  const animationFrameRef = useRef<number | undefined>(undefined)
  const explorerPanelRef = useRef<PanelImperativeHandle | null>(null)
  const hasMountedRef = useRef(false)
  const ignoreLayoutChangeRef = useRef(false)
  const splitLayoutRef = useRef<SplitLayout>(DEFAULT_SPLIT_LAYOUT)
  const visualizerPanelRef = useRef<PanelImperativeHandle | null>(null)

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

    window.cancelAnimationFrame(animationFrameRef.current ?? 0)
    setSplitAnimating(true)
    ignoreLayoutChangeRef.current = true

    const targetLayout = splitLayoutRef.current
    const startExplorer = getPanelPercentage(explorerPanel, splitOpen ? 100 : targetLayout.explorer)
    const startVisualizer = getPanelPercentage(visualizerPanel, splitOpen ? 0 : targetLayout.visualizer)
    const targetExplorer = splitOpen ? targetLayout.explorer : 100
    const targetVisualizer = splitOpen ? targetLayout.visualizer : 0
    const startedAt = performance.now()

    const animate = (frameTime: number) => {
      const progress = Math.min(1, (frameTime - startedAt) / SPLIT_ANIMATION_MS)
      const eased = easeOutCubic(progress)
      const explorerSize = interpolateNumber(startExplorer, targetExplorer, eased)
      const visualizerSize = interpolateNumber(startVisualizer, targetVisualizer, eased)

      explorerPanel.resize(`${explorerSize}%`)
      visualizerPanel.resize(`${visualizerSize}%`)

      if (progress < 1) {
        animationFrameRef.current = window.requestAnimationFrame(animate)
        return
      }

      explorerPanel.resize(`${targetExplorer}%`)
      if (splitOpen) {
        visualizerPanel.resize(`${targetVisualizer}%`)
      } else {
        visualizerPanel.collapse()
      }

      ignoreLayoutChangeRef.current = false
      setSplitAnimating(false)
    }

    animationFrameRef.current = window.requestAnimationFrame(animate)
    return () => {
      window.cancelAnimationFrame(animationFrameRef.current ?? 0)
    }
  }, [splitOpen])

  useEffect(() => {
    return () => {
      window.cancelAnimationFrame(animationFrameRef.current ?? 0)
    }
  }, [])

  return (
    <ResizablePanelGroup
      direction="horizontal"
      className={cn("h-full min-h-0 flex-1", (splitOpen || splitAnimating) && "gap-2")}
      defaultLayout={splitLayout}
      onLayoutChanged={(layout) => {
        if (
          ignoreLayoutChangeRef.current ||
          !splitOpen ||
          layout.visualizer === undefined ||
          layout.visualizer === 0
        ) {
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
        minSize="20%"
        panelRef={explorerPanelRef}
        className="flex min-h-0 min-w-0 overflow-hidden"
      >
        <FsExplorer
          selectedItemId={selectedItemId}
          visualizerOpen={splitOpen}
          onSelectionChange={setSelectedItemId}
          onVisualizerToggle={handleVisualizerToggle}
          onVisualizerSnapshotChange={handleVisualizerSnapshotChange}
        />
      </ResizablePanel>
      <ResizableHandle
        withHandle
        className={splitOpen && !splitAnimating ? "bg-transparent after:hidden" : "hidden"}
      />
      <ResizablePanel
        id="visualizer"
        collapsible
        collapsedSize="0%"
        defaultSize="0%"
        minSize={splitAnimating || !splitOpen ? "0%" : "25%"}
        panelRef={visualizerPanelRef}
        className="min-h-0 min-w-0 overflow-hidden"
      >
        <Visualizer
          colorScheme={colorScheme}
          selectedItemId={selectedItemId}
          snapshot={visualizerSnapshot}
          onSelectionChange={setSelectedItemId}
        />
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

function easeOutCubic(value: number) {
  return 1 - (1 - value) ** 3
}
