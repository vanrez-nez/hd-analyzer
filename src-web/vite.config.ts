import path from "node:path"
import { readFileSync } from "node:fs"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

const appVersion = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")).version

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
  server: {
    port: 1420,
    strictPort: true,
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
})
