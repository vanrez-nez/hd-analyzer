import { useEffect, useState } from "react"

export type ColorScheme = "light" | "dark"

const DARK_SCHEME_QUERY = "(prefers-color-scheme: dark)"

export function useSystemColorScheme(): ColorScheme {
  const [colorScheme, setColorScheme] = useState<ColorScheme>(() => getSystemColorScheme())

  useEffect(() => {
    const mediaQuery = window.matchMedia(DARK_SCHEME_QUERY)
    const syncColorScheme = () => {
      const nextColorScheme: ColorScheme = mediaQuery.matches ? "dark" : "light"
      document.documentElement.classList.toggle("dark", nextColorScheme === "dark")
      setColorScheme(nextColorScheme)
    }

    syncColorScheme()
    mediaQuery.addEventListener("change", syncColorScheme)
    return () => mediaQuery.removeEventListener("change", syncColorScheme)
  }, [])

  return colorScheme
}

function getSystemColorScheme(): ColorScheme {
  if (typeof window === "undefined") {
    return "light"
  }

  return window.matchMedia(DARK_SCHEME_QUERY).matches ? "dark" : "light"
}
