import React from "react"
import ReactDOM from "react-dom/client"
import { App } from "./App"
import { initializeLogging } from "./lib/logging"
import "./styles.css"

void initializeLogging()

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
