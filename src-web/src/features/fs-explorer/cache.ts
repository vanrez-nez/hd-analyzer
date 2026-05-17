import type { DirectoryListingDto, ExplorerCache } from "./types"

export function cacheKey(path: string, configFingerprint: string) {
  return `${configFingerprint}:${path}`
}

export function createExplorerCache(): ExplorerCache {
  return { listings: {} }
}

export function getCachedListing(
  cache: ExplorerCache,
  path: string,
  configFingerprint?: string,
): DirectoryListingDto | undefined {
  if (configFingerprint) {
    return cache.listings[cacheKey(path, configFingerprint)]
  }

  return Object.values(cache.listings).find((listing) => listing.path === path)
}

export function putCachedListing(cache: ExplorerCache, listing: DirectoryListingDto): ExplorerCache {
  const key = cacheKey(listing.path, listing.configFingerprint)
  const existing = cache.listings[key]

  if (existing && existing.generation > listing.generation) {
    return cache
  }

  const listings = integrateListingIntoAncestors(
    {
      ...cache.listings,
      [key]: listing,
    },
    listing,
  )

  return {
    listings,
  }
}

export function markPathStale(cache: ExplorerCache, path: string, descendants: boolean): ExplorerCache {
  const next = { ...cache.listings }

  for (const [key, listing] of Object.entries(next)) {
    const affected = descendants ? listing.path === path || listing.path.startsWith(`${path}/`) : listing.path === path
    if (affected) {
      next[key] = { ...listing, state: "stale" }
    }
  }

  return { listings: next }
}

function integrateListingIntoAncestors(
  listings: Record<string, DirectoryListingDto>,
  listing: DirectoryListingDto,
) {
  let current = listing
  let next = listings

  while (true) {
    const parentEntry = Object.entries(next).find(([, parent]) => {
      return (
        parent.configFingerprint === current.configFingerprint &&
        parent.children.some((child) => child.kind === "directory" && child.path === current.path)
      )
    })

    if (!parentEntry) {
      return next
    }

    const [parentKey, parent] = parentEntry
    const updatedParent = replaceChildDirectory(parent, current)
    if (updatedParent === parent) {
      return next
    }

    next = {
      ...next,
      [parentKey]: updatedParent,
    }
    current = updatedParent
  }
}

function replaceChildDirectory(parent: DirectoryListingDto, childListing: DirectoryListingDto) {
  const childIndex = parent.children.findIndex((child) => {
    return child.kind === "directory" && child.path === childListing.path
  })

  if (childIndex < 0) {
    return parent
  }

  const previousChild = parent.children[childIndex]
  const nextSize = childListing.totalMeasuredSize
  const nextLogicalSize = childListing.totalLogicalSize
  const nextChild = {
    ...previousChild,
    size: nextSize,
    logicalSize: nextLogicalSize,
    state: childListing.state,
    childrenKnown: !childListing.children.some((child) => child.visible),
    activeJobId: null,
    issues: childListing.issues,
  }
  const children = [...parent.children]
  children[childIndex] = nextChild

  return {
    ...parent,
    children,
    totalVisibleSize: previousChild.visible
      ? adjustedTotal(parent.totalVisibleSize, previousChild.size, nextSize)
      : parent.totalVisibleSize,
    totalMeasuredSize: adjustedTotal(parent.totalMeasuredSize, previousChild.size, nextSize),
    totalLogicalSize: adjustedTotal(parent.totalLogicalSize, previousChild.logicalSize, nextLogicalSize),
    generation: Math.max(parent.generation, childListing.generation),
  }
}

function adjustedTotal(total: number, previousValue: number, nextValue: number) {
  return Math.max(0, total - previousValue + nextValue)
}
