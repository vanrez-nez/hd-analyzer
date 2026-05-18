import { useEffect, useRef, useState } from "react"
import type { PanelImperativeHandle } from "react-resizable-panels"

import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable"
import { FsExplorer } from "@/features/fs-explorer/FsExplorer"
import { Visualizer } from "@/features/visualizer/Visualizer"

type SplitLayout = {
  explorer: number
  visualizer: number
}

const DEFAULT_SPLIT_LAYOUT: SplitLayout = {
  explorer: 30,
  visualizer: 70,
}

export function Layout() {
  const [splitOpen, setSplitOpen] = useState(false)
  const [splitLayout, setSplitLayout] = useState<SplitLayout>(DEFAULT_SPLIT_LAYOUT)
  const explorerPanelRef = useRef<PanelImperativeHandle | null>(null)
  const visualizerPanelRef = useRef<PanelImperativeHandle | null>(null)

  useEffect(() => {
    if (splitOpen) {
      explorerPanelRef.current?.resize(`${splitLayout.explorer}%`)
      visualizerPanelRef.current?.resize(`${splitLayout.visualizer}%`)
      return
    }

    visualizerPanelRef.current?.collapse()
    explorerPanelRef.current?.resize("100%")
  }, [splitLayout.explorer, splitLayout.visualizer, splitOpen])

  return (
    <ResizablePanelGroup
      direction="horizontal"
      className="h-full min-h-0 flex-1"
      defaultLayout={splitLayout}
      onLayoutChanged={(layout) => {
        if (!splitOpen || layout.visualizer === undefined || layout.visualizer === 0) {
          return
        }

        setSplitLayout({
          explorer: layout.explorer ?? DEFAULT_SPLIT_LAYOUT.explorer,
          visualizer: layout.visualizer,
        })
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
          visualizerOpen={splitOpen}
          onVisualizerToggle={() => setSplitOpen((open) => !open)}
        />
      </ResizablePanel>
      <ResizableHandle withHandle className={splitOpen ? "bg-transparent after:hidden" : "hidden"} />
      <ResizablePanel
        id="visualizer"
        collapsible
        collapsedSize="0%"
        defaultSize="0%"
        minSize="25%"
        panelRef={visualizerPanelRef}
        className="min-h-0 min-w-0 overflow-hidden"
      >
        <Visualizer />
      </ResizablePanel>
    </ResizablePanelGroup>
  )
}
