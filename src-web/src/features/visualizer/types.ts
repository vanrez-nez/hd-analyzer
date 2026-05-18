export type VisualizerCellInput = {
  id: string
  label: string
  path: string
  kind: "volume" | "directory" | "file" | "other"
  size: number
  state?: string
}

export type VisualizerLevelSnapshot = {
  path: string | null
  parentPath: string | null
  items: VisualizerCellInput[]
  generation: string
}
