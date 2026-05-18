import { useEffect, useRef, useState } from "react"

import { RafLoop } from "@/lib/raf-loop"

export function Visualizer() {
  const [fps, setFps] = useState(0)
  const frameCountRef = useRef(0)
  const sampleElapsedRef = useRef(0)

  useEffect(() => {
    const loop = new RafLoop((deltaSeconds) => {
      frameCountRef.current += 1
      sampleElapsedRef.current += deltaSeconds

      if (sampleElapsedRef.current >= 0.5) {
        setFps(Math.round(frameCountRef.current / sampleElapsedRef.current))
        frameCountRef.current = 0
        sampleElapsedRef.current = 0
      }
    })

    loop.start()
    return () => loop.stop()
  }, [])

  return (
    <div className="relative h-full w-full overflow-hidden">
      <canvas className="block h-full w-full" aria-label="Visualizer canvas" />
      <div className="pointer-events-none absolute right-2 top-2 text-xs tabular-nums text-muted-foreground">
        {fps} FPS
      </div>
    </div>
  )
}
