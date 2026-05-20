export type SelectionModifierInput = {
  ctrlKey?: boolean
  metaKey?: boolean
}

export function replaceSelection(itemIds: readonly string[]) {
  return uniqueSelection(itemIds)
}

export function toggleSelection(currentItemIds: readonly string[], itemIds: readonly string[]) {
  const uniqueIds = uniqueSelection(itemIds)
  if (uniqueIds.length === 0) {
    return uniqueSelection(currentItemIds)
  }

  const current = new Set(currentItemIds)
  const allSelected = uniqueIds.every((itemId) => current.has(itemId))

  if (allSelected) {
    uniqueIds.forEach((itemId) => current.delete(itemId))
    return Array.from(current)
  }

  uniqueIds.forEach((itemId) => current.add(itemId))
  return Array.from(current)
}

export function rangeSelection(visibleItemIds: readonly string[], anchorItemId: string | null, targetItemId: string) {
  const targetIndex = visibleItemIds.indexOf(targetItemId)
  const anchorIndex = anchorItemId ? visibleItemIds.indexOf(anchorItemId) : -1

  if (targetIndex === -1 || anchorIndex === -1) {
    return [targetItemId]
  }

  const start = Math.min(anchorIndex, targetIndex)
  const end = Math.max(anchorIndex, targetIndex)
  return visibleItemIds.slice(start, end + 1)
}

export function isToggleSelectionInput(input: SelectionModifierInput) {
  return Boolean(input.metaKey || input.ctrlKey)
}

function uniqueSelection(itemIds: readonly string[]) {
  return Array.from(new Set(itemIds.filter(Boolean)))
}
