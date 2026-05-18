declare module "d3-voronoi-map" {
  export type VoronoiMapPoint = [number, number]

  export type VoronoiMapPolygon<T> = VoronoiMapPoint[] & {
    site?: {
      originalObject?: {
        data?: {
          originalData?: T
        }
      }
    }
  }

  export type VoronoiMapState<T> = {
    ended: boolean
    iterationCount: number
    convergenceRatio: number
    polygons: VoronoiMapPolygon<T>[]
  }

  export type VoronoiMapSimulation<T> = {
    tick: () => void
    restart: () => VoronoiMapSimulation<T>
    stop: () => VoronoiMapSimulation<T>
    weight: (weight: (datum: T) => number) => VoronoiMapSimulation<T>
    convergenceRatio: (ratio: number) => VoronoiMapSimulation<T>
    maxIterationCount: (count: number) => VoronoiMapSimulation<T>
    minWeightRatio: (ratio: number) => VoronoiMapSimulation<T>
    clip: (clip: VoronoiMapPoint[]) => VoronoiMapSimulation<T>
    size: (size: [number, number]) => VoronoiMapSimulation<T>
    prng: (random: () => number) => VoronoiMapSimulation<T>
    initialPosition: (
      position: (datum: T, index: number, data: T[], simulation: VoronoiMapSimulation<T>) => VoronoiMapPoint,
    ) => VoronoiMapSimulation<T>
    state: () => VoronoiMapState<T>
    on: (eventName: "tick" | "end", callback?: () => void) => VoronoiMapSimulation<T>
  }

  export function voronoiMapSimulation<T>(data: T[]): VoronoiMapSimulation<T>

  export function voronoiMapInitialPositionPie<T>(): (
    datum: T,
    index: number,
    data: T[],
    simulation: VoronoiMapSimulation<T>,
  ) => VoronoiMapPoint
}
