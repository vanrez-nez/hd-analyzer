declare module "d3-voronoi-treemap" {
  export type VoronoiTreemapPoint = [number, number]

  export type VoronoiTreemapLayout = {
    (root: unknown): void
    convergenceRatio: (ratio: number) => VoronoiTreemapLayout
    maxIterationCount: (count: number) => VoronoiTreemapLayout
    minWeightRatio: (ratio: number) => VoronoiTreemapLayout
    clip: (clip: VoronoiTreemapPoint[]) => VoronoiTreemapLayout
    size: (size: [number, number]) => VoronoiTreemapLayout
    prng: (random: () => number) => VoronoiTreemapLayout
  }

  export function voronoiTreemap(): VoronoiTreemapLayout
}
