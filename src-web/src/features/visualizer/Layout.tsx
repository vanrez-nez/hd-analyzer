import { useMemo, useState } from "react"
import { PanelRightOpenIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable"
import { FsExplorer } from "@/features/fs-explorer/FsExplorer"
import { Visualizer } from "./Visualizer"

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
  const splitToggle = useMemo(
    () => (
      <Button
        type="button"
        variant={splitOpen ? "secondary" : "outline"}
        size="icon-sm"
        aria-label={splitOpen ? "Hide visualizer" : "Show visualizer"}
        aria-pressed={splitOpen}
        onClick={() => setSplitOpen((open) => !open)}
      >
        <PanelRightOpenIcon data-icon="inline-start" />
      </Button>
    ),
    [splitOpen],
  )

  return (
    <ResizablePanelGroup
      direction="horizontal"
      className="h-full min-h-0 flex-1"
      defaultLayout={splitOpen ? splitLayout : { explorer: 100 }}
      onLayoutChanged={(layout) => {
        if (!splitOpen || layout.visualizer === undefined) {
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
        defaultSize={splitOpen ? splitLayout.explorer : 100}
        minSize={splitOpen ? "20%" : "100%"}
        className="flex min-h-0 min-w-0 overflow-hidden"
      >
        <FsExplorer headerActions={splitToggle} tableClassName={splitOpen ? "rounded-r-none border-r-0" : undefined} />
      </ResizablePanel>
      {splitOpen ? (
        <>
          <ResizableHandle withHandle />
          <ResizablePanel
            id="visualizer"
            defaultSize={splitLayout.visualizer}
            minSize="25%"
            className="min-h-0 min-w-0 overflow-hidden"
          >
            <Visualizer />
          </ResizablePanel>
        </>
      ) : null}
    </ResizablePanelGroup>
  )
}
