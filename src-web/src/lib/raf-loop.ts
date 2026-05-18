export type RafLoopTick = (deltaSeconds: number, elapsedSeconds: number, timestamp: number) => void

type RafLoopScheduler = (callback: FrameRequestCallback) => number
type RafLoopCancel = (handle: number) => void
type RafLoopNow = () => number

export type RafLoopOptions = {
  autoStart?: boolean
  cancelFrame?: RafLoopCancel
  maxDeltaSeconds?: number
  now?: RafLoopNow
  requestFrame?: RafLoopScheduler
}

const DEFAULT_MAX_DELTA_SECONDS = 0.1

export class RafLoop {
  private frameHandle: number | undefined
  private lastTimestamp: number | undefined
  private elapsedSeconds = 0
  private paused = false
  private running = false
  private readonly cancelFrame: RafLoopCancel
  private readonly maxDeltaSeconds: number
  private readonly now: RafLoopNow
  private readonly requestFrame: RafLoopScheduler
  private readonly tick: RafLoopTick

  constructor(tick: RafLoopTick, options: RafLoopOptions = {}) {
    this.tick = tick
    this.requestFrame = options.requestFrame ?? window.requestAnimationFrame.bind(window)
    this.cancelFrame = options.cancelFrame ?? window.cancelAnimationFrame.bind(window)
    this.now = options.now ?? window.performance.now.bind(window.performance)
    this.maxDeltaSeconds = options.maxDeltaSeconds ?? DEFAULT_MAX_DELTA_SECONDS

    if (options.autoStart) {
      this.start()
    }
  }

  get isPaused() {
    return this.paused
  }

  get isRunning() {
    return this.running
  }

  get elapsed() {
    return this.elapsedSeconds
  }

  start() {
    if (this.running) {
      return
    }

    this.running = true
    this.paused = false
    this.elapsedSeconds = 0
    this.lastTimestamp = this.now()
    this.schedule()
  }

  stop() {
    if (this.frameHandle !== undefined) {
      this.cancelFrame(this.frameHandle)
      this.frameHandle = undefined
    }

    this.running = false
    this.paused = false
    this.lastTimestamp = undefined
    this.elapsedSeconds = 0
  }

  pause() {
    if (!this.running || this.paused) {
      return
    }

    this.paused = true
    if (this.frameHandle !== undefined) {
      this.cancelFrame(this.frameHandle)
      this.frameHandle = undefined
    }
  }

  resume() {
    if (!this.running || !this.paused) {
      return
    }

    this.paused = false
    this.lastTimestamp = this.now()
    this.schedule()
  }

  private schedule() {
    this.frameHandle = this.requestFrame(this.handleFrame)
  }

  private readonly handleFrame = (timestamp: number) => {
    this.frameHandle = undefined
    if (!this.running || this.paused) {
      return
    }

    const previousTimestamp = this.lastTimestamp ?? timestamp
    const deltaSeconds = Math.min(
      Math.max(0, (timestamp - previousTimestamp) / 1000),
      this.maxDeltaSeconds,
    )
    this.lastTimestamp = timestamp
    this.elapsedSeconds += deltaSeconds
    this.tick(deltaSeconds, this.elapsedSeconds, timestamp)
    this.schedule()
  }
}
