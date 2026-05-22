import { voronoiTreemap } from "d3-voronoi-treemap"

import {
  fileColorForGroup,
  fileColorGroupForInput,
  fileExtension,
} from "@/file-colors"
import type { FileColorGroup } from "@/file-colors"
import type { ColorScheme } from "@/lib/use-system-color-scheme"
import type { VisualizerCellInput, VisualizerLevelSnapshot } from "./types"

type PartitionType = "size" | "type"
type FileItemType = string
type OverflowReason = "visibility" | "cardinality" | null

type FileItem = {
  id: string
  weight: number
  type?: FileItemType
  source: VisualizerCellInput
}

type VoronoiSite = {
  id: string
  weight: number
  members: FileItem[]
  isCluster: boolean
  representative: FileItem
  sizeRange: [number, number]
  overflowReason: OverflowReason
  mixed: boolean
}

type ClusterParams = {
  visibilityThreshold?: number
  logGapThreshold?: number
  maxItemsPerCluster?: number
  maxSites?: number
  partitionByType?: boolean
}

type NormalizedClusterParams = {
  visibilityThreshold: number
  logGapThreshold: number
  maxItemsPerCluster: number
  maxSites: number
  partitionByType: boolean
}

type ClusterResult = {
  sites: VoronoiSite[]
  dropped: FileItem[]
  totalWeight: number
  minVisible: number
  overflowCount: number
}

export type VisualizerPoint = [number, number]

export type VisualizerCircle = {
  centerX: number
  centerY: number
  radius: number
}

export type VisualizerLayoutCell = {
  id: string
  label: string
  path: string
  kind: VisualizerCellInput["kind"]
  memberIds: string[]
  selectionId: string
  size: number
  state?: string
  polygon: VisualizerPoint[]
  colorGroup: FileColorGroup
  fillColor: string
}

export type VisualizerLayout = {
  generation: string
  path: string | null
  colorScheme: ColorScheme
  circle: VisualizerCircle
  cells: VisualizerLayoutCell[]
}

type VoronoiPoint = VisualizerPoint

type RenderCell = {
  site: VoronoiSite
  item?: FileItem
  polygon: VoronoiPoint[]
}

type TreemapNodeData = {
  site?: VoronoiSite
  item?: FileItem
}

type TreemapNode = {
  children?: TreemapNode[]
  data: TreemapNodeData
  depth: number
  height: number
  parent: TreemapNode | null
  polygon?: VoronoiPoint[]
  value: number
}

type TreemapLeafNode = TreemapNode & {
  data: TreemapNodeData & {
    site: VoronoiSite
  }
  polygon?: VoronoiPoint[]
}

type CircleBounds = VisualizerCircle

const MIN_AREA = 0.01
const LOG_SPREAD = 4
const MAX_MEMBERS = 64
const MAX_SITES = 200
const PARTITION_TYPE: PartitionType = "size"
const ITERATIONS = 50
const CONVERGENCE = 0.02
const VORONOI_MIN_WEIGHT_RATIO = 0.00001

const DEFAULT_PARAMS: NormalizedClusterParams = {
  visibilityThreshold: MIN_AREA,
  logGapThreshold: LOG_SPREAD,
  maxItemsPerCluster: MAX_MEMBERS,
  maxSites: MAX_SITES,
  partitionByType: isTypePartition(PARTITION_TYPE),
}
const DEFAULT_SEED = 0x5eed
const CIRCLE_SEGMENTS = 196
const CIRCLE_FRAME_STROKE_WIDTH = 2
const UNTYPED = Symbol("untyped")

function isTypePartition(partitionType: PartitionType) {
  return partitionType === "type"
}

export class Voronoi {
  private readonly circle: CircleBounds
  private currentLayout: VisualizerLayout
  private readonly fullBoundary: VoronoiPoint[]
  private readonly height: number
  private readonly width: number

  constructor(width: number, height: number, snapshot: VisualizerLevelSnapshot, colorScheme: ColorScheme) {
    this.width = Math.max(1, width)
    this.height = Math.max(1, height)
    this.circle = createCircleBounds(this.width, this.height)
    this.fullBoundary = createCircleBoundary(this.circle)
    this.currentLayout =
      createLayout(snapshot, this.fullBoundary, this.circle, colorScheme) ??
      createEmptyLayout(snapshot, this.circle, colorScheme)
  }

  setSnapshot(snapshot: VisualizerLevelSnapshot, colorScheme: ColorScheme) {
    if (
      snapshot.path === this.currentLayout.path &&
      snapshot.generation === this.currentLayout.generation &&
      colorScheme === this.currentLayout.colorScheme
    ) {
      return
    }

    const nextLayout = createLayout(snapshot, this.fullBoundary, this.circle, colorScheme)
    if (nextLayout) {
      this.currentLayout = nextLayout
    }
  }

  getLayout(snapshot: VisualizerLevelSnapshot, colorScheme: ColorScheme) {
    this.setSnapshot(snapshot, colorScheme)
    return this.currentLayout
  }

  draw(context: CanvasRenderingContext2D) {
    this.drawBackground(context)
    this.drawDebugBase(context)
    this.drawFrame(context)
    return false
  }

  drawBackground(context: CanvasRenderingContext2D) {
    context.clearRect(0, 0, this.width, this.height)
    drawCircleBackground(context, this.circle)
  }

  drawDebugBase(context: CanvasRenderingContext2D, layout = this.currentLayout) {
    drawContainedCells(context, layout.circle, layout.cells)
  }

  drawFrame(context: CanvasRenderingContext2D, layout = this.currentLayout) {
    drawCircleFrame(context, layout.circle)
  }
}

function createLayout(
  snapshot: VisualizerLevelSnapshot,
  boundary: VoronoiPoint[],
  circle: CircleBounds,
  colorScheme: ColorScheme,
): VisualizerLayout | undefined {
  const clusterResult = clusterForVoronoi(createFileItems(snapshot.items))
  const renderCells = createCells(snapshot, boundary, clusterResult)
  if (!renderCells) {
    return undefined
  }

  return {
    generation: snapshot.generation,
    path: snapshot.path,
    colorScheme,
    circle,
    cells: renderCells.map((cell) => createLayoutCell(cell, colorScheme)),
  }
}

function createEmptyLayout(
  snapshot: VisualizerLevelSnapshot,
  circle: CircleBounds,
  colorScheme: ColorScheme,
): VisualizerLayout {
  return {
    generation: snapshot.generation,
    path: snapshot.path,
    colorScheme,
    circle,
    cells: [],
  }
}

function createCells(
  snapshot: VisualizerLevelSnapshot,
  boundary: VoronoiPoint[],
  clusterResult: ClusterResult,
): RenderCell[] | undefined {
  if (clusterResult.sites.length === 0) {
    return []
  }

  if (clusterResult.sites.length === 1) {
    return [createRenderCell(clusterResult.sites[0], boundary, clusterResult.sites[0].representative)]
  }

  const debugInput = createVoronoiDebugInput(snapshot, clusterResult)
  console.groupCollapsed("[visualizer] voronoi structure")
  console.log(debugInput.summary)
  console.table(debugInput.largestSites)
  console.table(debugInput.smallestSites)

  try {
    const root = createTreemapRoot(clusterResult.sites)

    const treemap = voronoiTreemap()
      .clip(boundary)
      .minWeightRatio(VORONOI_MIN_WEIGHT_RATIO)
      .convergenceRatio(CONVERGENCE)
      .maxIterationCount(ITERATIONS)
      .prng(createRandom(hashLayoutSeed(snapshot)))
    treemap(root)

    const children = root.children ?? []
    const leaves = collectLeafNodes(root)
    const childrenWithPolygons = children.filter((node) => node.polygon?.length)
    const childrenMissingPolygons = children.filter((node) => !node.polygon?.length)
    const leavesWithPolygons = leaves.filter((node) => node.polygon?.length)
    const leavesMissingPolygons = leaves.filter((node) => !node.polygon?.length)
    const cells = leaves
      .map((node) => {
        return node.polygon ? createRenderCell(node.data.site, node.polygon, node.data.item) : undefined
      })
      .filter((cell): cell is RenderCell => Boolean(cell))

    console.log({
      root,
      rootValue: root.value,
      childCount: children.length,
      childrenWithPolygons: childrenWithPolygons.length,
      childrenMissingPolygons: childrenMissingPolygons.length,
      leafCount: leaves.length,
      leavesWithPolygons: leavesWithPolygons.length,
      leavesMissingPolygons: leavesMissingPolygons.length,
      sampleChildren: children.slice(0, 10).map(summarizeTreemapNode),
      sampleLeaves: leaves.slice(0, 10).map(summarizeTreemapNode),
    })
    console.groupEnd()

    return cells
  } catch (error) {
    console.error("[visualizer] voronoi layout failed", {
      path: snapshot.path,
      siteCount: clusterResult.sites.length,
      clusterCount: clusterResult.sites.filter((site) => site.isCluster).length,
      overflowCount: clusterResult.overflowCount,
      minVisible: clusterResult.minVisible,
      error,
    })
    console.groupEnd()
    return undefined
  }
}

function createRenderCell(site: VoronoiSite, polygon: VoronoiPoint[], item?: FileItem): RenderCell {
  return {
    site,
    item,
    polygon: polygon.map(([x, y]): VoronoiPoint => [x, y]),
  }
}

function createLayoutCell(cell: RenderCell, colorScheme: ColorScheme): VisualizerLayoutCell {
  const source = cell.item?.source ?? cell.site.representative.source
  const colorGroup = colorGroupForRenderCell(cell)
  const directItem = cell.site.overflowReason ? undefined : cell.item
  const selectionId = directItem ? directItem.id : cell.site.id

  return {
    id: selectionId,
    label: source.label,
    path: source.path,
    kind: source.kind,
    memberIds: directItem ? [directItem.id] : cell.site.members.map((member) => member.id),
    selectionId,
    size: cell.item?.weight ?? cell.site.weight,
    state: source.state,
    polygon: cell.polygon.map(([x, y]): VisualizerPoint => [x, y]),
    colorGroup,
    fillColor: fileColorForGroup(colorGroup, colorScheme),
  }
}

function createTreemapRoot(sites: VoronoiSite[]): TreemapNode {
  const layoutSites = sortSitesByLayoutKey(sites)
  const root: TreemapNode = {
    children: [],
    data: {},
    depth: 0,
    height: 0,
    parent: null,
    value: layoutSites.reduce((total, site) => total + site.weight, 0),
  }

  root.children = layoutSites.map((site) => createSiteNode(site, root, 1))
  root.height = root.children.length > 0 ? Math.max(...root.children.map((child) => child.height)) + 1 : 0
  return root
}

function createSiteNode(site: VoronoiSite, parent: TreemapNode, depth: number): TreemapNode {
  const shouldRenderAsClusterCell = !site.isCluster || Boolean(site.overflowReason)
  const node: TreemapNode = {
    data: {
      site,
      item: shouldRenderAsClusterCell ? site.representative : undefined,
    },
    depth,
    height: shouldRenderAsClusterCell ? 0 : 1,
    parent,
    value: site.weight,
  }

  if (!shouldRenderAsClusterCell) {
    node.children = sortItemsByLayoutKey(site.members).map((item) => createItemNode(site, item, node, depth + 1))
  }

  return node
}

function createItemNode(site: VoronoiSite, item: FileItem, parent: TreemapNode, depth: number): TreemapNode {
  return {
    data: {
      site,
      item,
    },
    depth,
    height: 0,
    parent,
    value: item.weight,
  }
}

function collectLeafNodes(node: TreemapNode): TreemapLeafNode[] {
  if (!node.children?.length) {
    return node.data.site ? [node as TreemapLeafNode] : []
  }

  return node.children.flatMap(collectLeafNodes)
}

function createFileItems(items: VisualizerCellInput[]): FileItem[] {
  return items.map((item) => ({
    id: item.id,
    weight: Number.isFinite(item.size) && item.size > 0 ? item.size : 0,
    type: extensionType(item),
    source: item,
  }))
}

function clusterForVoronoi(items: FileItem[], params: ClusterParams = {}): ClusterResult {
  const config = normalizeParams(params)
  const valid: FileItem[] = []
  const dropped: FileItem[] = []

  for (const item of items) {
    assertItem(item)

    if (item.weight === 0) {
      dropped.push(item)
    } else {
      valid.push(item)
    }
  }

  const totalWeight = sumWeight(valid)
  if (totalWeight === 0) {
    return {
      sites: [],
      dropped,
      totalWeight: 0,
      minVisible: 0,
      overflowCount: 0,
    }
  }

  const minVisible = totalWeight * config.visibilityThreshold
  const large: FileItem[] = []
  const small: FileItem[] = []

  for (const item of valid) {
    if (item.weight >= minVisible) {
      large.push(item)
    } else {
      small.push(item)
    }
  }

  const singletonSites = large.map(toSingletonSite)
  let clusterSites: VoronoiSite[] = []

  if (small.length > 0) {
    clusterSites = config.partitionByType
      ? clusterSmallItemsByType(small, minVisible, config)
      : clusterSmallItems(small, minVisible, config)
  }

  const sites = enforceSiteBudget([...singletonSites, ...clusterSites], config.maxSites, minVisible)
  const overflowCount = sites.reduce((count, site) => count + (site.overflowReason ? 1 : 0), 0)

  return {
    sites,
    dropped,
    totalWeight,
    minVisible,
    overflowCount,
  }
}

function normalizeParams(params: ClusterParams = {}): NormalizedClusterParams {
  const config = {
    ...DEFAULT_PARAMS,
    ...params,
  }

  if (
    !Number.isFinite(config.visibilityThreshold) ||
    config.visibilityThreshold <= 0 ||
    config.visibilityThreshold > 1
  ) {
    throw new RangeError("visibilityThreshold must be > 0 and <= 1")
  }

  if (!Number.isFinite(config.logGapThreshold) || config.logGapThreshold < 0) {
    throw new RangeError("logGapThreshold must be >= 0")
  }

  if (!Number.isInteger(config.maxItemsPerCluster) || config.maxItemsPerCluster < 1) {
    throw new RangeError("maxItemsPerCluster must be a positive integer")
  }

  if (!Number.isInteger(config.maxSites) || config.maxSites < 1) {
    throw new RangeError("maxSites must be a positive integer")
  }

  config.partitionByType = Boolean(config.partitionByType)
  return config
}

function clusterSmallItemsByType(
  items: FileItem[],
  minVisible: number,
  params: NormalizedClusterParams,
) {
  const byType = new Map<FileItemType | typeof UNTYPED, FileItem[]>()

  for (const item of items) {
    const key = item.type === undefined ? UNTYPED : item.type
    const group = byType.get(key) || []
    group.push(item)
    byType.set(key, group)
  }

  const clusters: VoronoiSite[] = []
  const spill: FileItem[] = []

  for (const group of byType.values()) {
    const groupClusters = clusterSmallItems(group, minVisible, params)

    for (const cluster of groupClusters) {
      if (cluster.weight < minVisible) {
        spill.push(...cluster.members)
      } else {
        clusters.push(cluster)
      }
    }
  }

  if (spill.length > 0) {
    clusters.push(
      ...clusterSmallItems(spill, minVisible, params).map((site) => ({
        ...site,
        mixed: true,
      })),
    )
  }

  return clusters
}

function clusterSmallItems(items: FileItem[], minVisible: number, params: NormalizedClusterParams) {
  const sorted = stableSortByWeightAsc(items)
  let clusters: VoronoiSite[] = []
  let current: FileItem[] = []
  let currentWeight = 0
  let previousLog = -Infinity

  for (const item of sorted) {
    const itemLog = Math.log2(item.weight + 1)
    const currentIsVisible = currentWeight >= minVisible
    const tooSpread = itemLog - previousLog > params.logGapThreshold
    const tooMany = current.length >= params.maxItemsPerCluster

    if (current.length > 0 && currentIsVisible && (tooSpread || tooMany)) {
      clusters.push(makeSite(current, currentWeight))
      current = []
      currentWeight = 0
    }

    current.push(item)
    currentWeight += item.weight
    previousLog = itemLog
  }

  if (current.length > 0) {
    clusters.push(makeSite(current, currentWeight))
  }

  clusters = mergeSubthreshold(clusters, minVisible)
  clusters = enforceCardinality(clusters, minVisible, params.maxItemsPerCluster)
  return clusters
}

function mergeSubthreshold(clusters: VoronoiSite[], minVisible: number) {
  if (clusters.length <= 1) {
    return clusters
  }

  const result: VoronoiSite[] = []

  for (const cluster of clusters) {
    const last = result[result.length - 1]

    if (last && last.weight < minVisible) {
      result[result.length - 1] = mergeSites(last, cluster, minVisible)
    } else {
      result.push(cluster)
    }
  }

  if (result.length >= 2 && result[result.length - 1].weight < minVisible) {
    const tail = result.pop()
    if (tail) {
      result[result.length - 1] = mergeSites(result[result.length - 1], tail, minVisible)
    }
  }

  return result
}

function enforceCardinality(clusters: VoronoiSite[], minVisible: number, maxItems: number) {
  const result: VoronoiSite[] = []

  for (const cluster of clusters) {
    if (cluster.weight < minVisible) {
      result.push({
        ...cluster,
        overflowReason: "visibility",
      })
      continue
    }

    if (cluster.members.length <= maxItems) {
      result.push(cluster)
      continue
    }

    const chunks = splitSiteIntoVisibleChunks(cluster, minVisible, maxItems)

    if (chunks === null) {
      result.push({
        ...cluster,
        overflowReason: "cardinality",
      })
    } else {
      result.push(...chunks)
    }
  }

  return result
}

function splitSiteIntoVisibleChunks(site: VoronoiSite, minVisible: number, maxItems: number) {
  const targetSize = Math.max(1, Math.floor(maxItems * 0.75))
  const chunkCount = Math.ceil(site.members.length / targetSize)
  const chunkSize = Math.ceil(site.members.length / chunkCount)
  const ascending = stableSortByWeightAsc(site.members)
  const chunks: VoronoiSite[] = []

  for (let index = 0; index < ascending.length; index += chunkSize) {
    const members = ascending.slice(index, index + chunkSize)
    const chunk = makeSite(members, sumWeight(members))

    if (chunk.weight < minVisible) {
      return null
    }

    chunks.push(chunk)
  }

  return chunks
}

function enforceSiteBudget(sites: VoronoiSite[], maxSites: number, minVisible: number) {
  if (sites.length <= maxSites) {
    return sortSites(sites)
  }

  const sorted = sortSites(sites)
  const singletons = sorted.filter((site) => !site.isCluster)
  const clusters = sorted.filter((site) => site.isCluster)

  while (singletons.length + clusters.length > maxSites && clusters.length >= 2) {
    let bestPair = 0
    let bestGap = Infinity

    for (let index = 0; index + 1 < clusters.length; index += 1) {
      const currentGap = Math.abs(
        Math.log2(clusters[index + 1].weight) - Math.log2(clusters[index].weight),
      )

      if (currentGap < bestGap) {
        bestGap = currentGap
        bestPair = index
      }
    }

    const merged = mergeSites(clusters[bestPair], clusters[bestPair + 1], minVisible)
    clusters.splice(bestPair, 2, merged)
    clusters.sort(compareSiteWeightDesc)
  }

  return sortSites([...singletons, ...clusters])
}

function toSingletonSite(item: FileItem) {
  return makeSite([item], item.weight)
}

function makeSite(members: FileItem[], weight = sumWeight(members)) {
  const sortedMembers = stableSortByWeightDesc(members)
  return makeSiteFromSortedMembers(sortedMembers, weight)
}

function makeSiteFromSortedMembers(sortedMembers: FileItem[], weight = sumWeight(sortedMembers)): VoronoiSite {
  const min = sortedMembers.reduce((value, item) => (item.weight < value ? item.weight : value), Infinity)
  const max = sortedMembers.reduce((value, item) => (item.weight > value ? item.weight : value), -Infinity)
  const id =
    sortedMembers.length === 1
      ? sortedMembers[0].id
      : `cluster:${hashMemberIds(sortedMembers)}:${sortedMembers.length}`

  return {
    id,
    weight,
    members: sortedMembers,
    isCluster: sortedMembers.length > 1,
    representative: sortedMembers[0],
    sizeRange: [min, max],
    overflowReason: null,
    mixed: hasMixedTypes(sortedMembers),
  }
}

function mergeSites(left: VoronoiSite, right: VoronoiSite, minVisible = 0) {
  const merged = makeSiteFromSortedMembers(
    mergeMembersByWeightDesc(left.members, right.members),
    left.weight + right.weight,
  )
  merged.overflowReason = mergeOverflowReason(left, right, merged.weight, minVisible)
  merged.mixed = Boolean(left.mixed || right.mixed || merged.mixed)
  return merged
}

function sortSites(sites: VoronoiSite[]) {
  return [...sites].sort(compareSiteWeightDesc)
}

function sortSitesByLayoutKey(sites: VoronoiSite[]) {
  return [...sites].sort((left, right) => left.id.localeCompare(right.id))
}

function sortItemsByLayoutKey(items: FileItem[]) {
  return [...items].sort((left, right) => left.id.localeCompare(right.id))
}

function compareSiteWeightDesc(a: VoronoiSite, b: VoronoiSite) {
  return b.weight - a.weight || siteSortId(a).localeCompare(siteSortId(b))
}

function stableSortByWeightAsc(items: FileItem[]) {
  return [...items].sort((a, b) => a.weight - b.weight || String(a.id).localeCompare(String(b.id)))
}

function stableSortByWeightDesc(items: FileItem[]) {
  return [...items].sort((a, b) => b.weight - a.weight || String(a.id).localeCompare(String(b.id)))
}

function siteSortId(site: VoronoiSite) {
  return site.representative ? String(site.representative.id) : site.id
}

function mergeOverflowReason(left: VoronoiSite, right: VoronoiSite, weight: number, minVisible: number): OverflowReason {
  if (left.overflowReason === "cardinality" || right.overflowReason === "cardinality") {
    return "cardinality"
  }

  if (left.overflowReason === "visibility" || right.overflowReason === "visibility") {
    return weight < minVisible ? "visibility" : null
  }

  return null
}

function mergeMembersByWeightDesc(left: FileItem[], right: FileItem[]) {
  const merged: FileItem[] = []
  let leftIndex = 0
  let rightIndex = 0

  while (leftIndex < left.length && rightIndex < right.length) {
    const leftItem = left[leftIndex]
    const rightItem = right[rightIndex]

    if (
      leftItem.weight > rightItem.weight ||
      (leftItem.weight === rightItem.weight && String(leftItem.id) <= String(rightItem.id))
    ) {
      merged.push(leftItem)
      leftIndex += 1
    } else {
      merged.push(rightItem)
      rightIndex += 1
    }
  }

  while (leftIndex < left.length) {
    merged.push(left[leftIndex])
    leftIndex += 1
  }

  while (rightIndex < right.length) {
    merged.push(right[rightIndex])
    rightIndex += 1
  }

  return merged
}

function hasMixedTypes(members: FileItem[]) {
  if (members.length <= 1) {
    return false
  }

  const firstType = members[0].type
  for (let index = 1; index < members.length; index += 1) {
    if (members[index].type !== firstType) {
      return true
    }
  }

  return false
}

function hashMemberIds(members: FileItem[]) {
  let hash = 2166136261

  for (const member of sortItemsByLayoutKey(members)) {
    const id = String(member.id)
    for (let index = 0; index < id.length; index += 1) {
      hash ^= id.charCodeAt(index)
      hash = Math.imul(hash, 16777619)
    }
    hash ^= 124
    hash = Math.imul(hash, 16777619)
  }

  return (hash >>> 0).toString(36)
}

function sumWeight(items: FileItem[]) {
  return items.reduce((sum, item) => sum + item.weight, 0)
}

function assertItem(item: FileItem) {
  if (!item || typeof item.id !== "string") {
    throw new RangeError("FileItem.id must be a string")
  }

  if (!Number.isFinite(item.weight) || item.weight < 0) {
    throw new RangeError("FileItem.weight must be a finite number >= 0")
  }
}

function extensionType(item: VisualizerCellInput): string {
  switch (item.kind) {
    case "volume":
      return "volume"
    case "directory":
      return "directory"
    case "other":
      return "other"
    case "file":
      return fileExtension(item.label || item.path)
  }
}

function colorGroupForRenderCell(cell: RenderCell): FileColorGroup {
  if (cell.item && !cell.site.overflowReason) {
    return colorGroupForItem(cell.item)
  }

  return colorGroupForSite(cell.site)
}

function colorGroupForSite(site: VoronoiSite): FileColorGroup {
  const first = colorGroupForItem(site.representative)

  for (const member of site.members) {
    if (colorGroupForItem(member) !== first) {
      return "file"
    }
  }

  return first
}

function colorGroupForItem(item: FileItem): FileColorGroup {
  return fileColorGroupForInput(item.source)
}

function createVoronoiDebugInput(snapshot: VisualizerLevelSnapshot, clusterResult: ClusterResult) {
  const valid = clusterResult.sites.flatMap((site) => site.members)
  const weights = valid.map((item) => item.weight)
  const sortedSites = [...clusterResult.sites].sort((left, right) => right.weight - left.weight)

  return {
    summary: {
      path: snapshot.path,
      parentPath: snapshot.parentPath,
      rawItemCount: snapshot.items.length,
      validItemCount: valid.length,
      droppedZeroWeightCount: clusterResult.dropped.length,
      siteCount: clusterResult.sites.length,
      clusterCount: clusterResult.sites.filter((site) => site.isCluster).length,
      overflowCount: clusterResult.overflowCount,
      minVisible: clusterResult.minVisible,
      totalWeight: clusterResult.totalWeight,
      minWeight: weights.length > 0 ? Math.min(...weights) : 0,
      maxWeight: weights.length > 0 ? Math.max(...weights) : 0,
      partitionType: PARTITION_TYPE,
    },
    largestSites: sortedSites.slice(0, 10).map(summarizeSite),
    smallestSites: sortedSites.slice(-10).reverse().map(summarizeSite),
  }
}

function summarizeSite(site: VoronoiSite) {
  return {
    id: site.id,
    weight: site.weight,
    memberCount: site.members.length,
    isCluster: site.isCluster,
    representativeId: site.representative.id,
    representativeLabel: site.representative.source.label,
    representativeType: site.representative.type,
    sizeRangeMin: site.sizeRange[0],
    sizeRangeMax: site.sizeRange[1],
    overflowReason: site.overflowReason,
    mixed: site.mixed,
  }
}

function summarizeTreemapNode(node: TreemapNode) {
  return {
    depth: node.depth,
    height: node.height,
    value: node.value,
    site: node.data.site ? summarizeSite(node.data.site) : undefined,
    item: node.data.item
      ? {
          id: node.data.item.id,
          label: node.data.item.source.label,
          type: node.data.item.type,
          weight: node.data.item.weight,
        }
      : undefined,
    polygonPoints: node.polygon?.length ?? 0,
    polygonSample: node.polygon?.slice(0, 3),
  }
}

function createCircleBounds(width: number, height: number): CircleBounds {
  const radiusInset = CIRCLE_FRAME_STROKE_WIDTH / 2
  return {
    centerX: width / 2,
    centerY: height / 2,
    radius: Math.max(1, Math.min(width, height) / 2 - radiusInset),
  }
}

function createCircleBoundary(circle: CircleBounds) {
  return Array.from({ length: CIRCLE_SEGMENTS }, (_, index): VoronoiPoint => {
    const angle = -Math.PI / 2 - (index / CIRCLE_SEGMENTS) * Math.PI * 2
    return [
      circle.centerX + Math.cos(angle) * circle.radius,
      circle.centerY + Math.sin(angle) * circle.radius,
    ]
  })
}

function drawCircleBackground(context: CanvasRenderingContext2D, circle: CircleBounds) {
  context.save()
  drawCirclePath(context, circle)
  context.fillStyle = "hsl(0 0% 0% / 0.02)"
  context.fill()
  context.restore()
}

function drawCircleFrame(context: CanvasRenderingContext2D, circle: CircleBounds) {
  context.save()
  drawCirclePath(context, circle)
  context.strokeStyle = "hsl(0 0% 100% / 0.16)"
  context.lineWidth = CIRCLE_FRAME_STROKE_WIDTH
  context.stroke()
  context.restore()
}

function drawContainedCells(
  context: CanvasRenderingContext2D,
  circle: CircleBounds,
  cells: VisualizerLayoutCell[],
) {
  context.save()
  drawCirclePath(context, circle)
  context.clip()
  cells.forEach((cell) => drawPolygon(context, cell.polygon, cell.fillColor))
  context.restore()
}

function drawPolygon(context: CanvasRenderingContext2D, polygon: VoronoiPoint[], fillStyle: string) {
  if (polygon.length === 0) {
    return
  }

  context.beginPath()
  polygon.forEach(([x, y], index) => {
    if (index === 0) {
      context.moveTo(x, y)
      return
    }

    context.lineTo(x, y)
  })
  context.closePath()
  context.fillStyle = fillStyle
  context.fill()
  context.strokeStyle = "hsl(0 0% 100% / 0.12)"
  context.lineWidth = 1
  context.stroke()
}

function drawCirclePath(context: CanvasRenderingContext2D, circle: CircleBounds) {
  context.beginPath()
  context.arc(circle.centerX, circle.centerY, circle.radius, 0, Math.PI * 2)
}

function hashLayoutSeed(snapshot: VisualizerLevelSnapshot) {
  return (hashString(snapshot.path ?? "volumes") ^ DEFAULT_SEED) >>> 0
}

function hashString(value: string) {
  let hash = 2166136261
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }

  return hash >>> 0
}

function createRandom(seed: number) {
  let state = seed >>> 0
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0
    return state / 0x100000000
  }
}
