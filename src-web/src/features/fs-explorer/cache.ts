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

  return {
    listings: {
      ...cache.listings,
      [key]: listing,
    },
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
