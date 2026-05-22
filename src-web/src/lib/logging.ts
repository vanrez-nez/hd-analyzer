import {
  attachConsole,
  debug as logDebug,
  error as logError,
  info as logInfo,
  warn as logWarn,
} from "@tauri-apps/plugin-log"

type LogContext = Record<string, unknown>
type LogWriter = (message: string) => Promise<void>

let initialized = false
let pluginAvailable = true
const isDev = Boolean((import.meta as ImportMeta & { env?: { DEV?: boolean } }).env?.DEV)

export async function initializeLogging() {
  if (initialized) return
  initialized = true

  try {
    await attachConsole()
    await logInfo("Space Lenser webview logging initialized")
  } catch (error) {
    pluginAvailable = false
    if (isDev) {
      console.debug("Tauri logging plugin is unavailable in this runtime.", error)
    }
  }
}

export const appLog = {
  debug: (message: string, context?: LogContext) => writeLog(logDebug, message, context),
  error: (message: string, context?: LogContext) => writeLog(logError, message, context),
  info: (message: string, context?: LogContext) => writeLog(logInfo, message, context),
  warn: (message: string, context?: LogContext) => writeLog(logWarn, message, context),
}

async function writeLog(writer: LogWriter, message: string, context?: LogContext) {
  if (!pluginAvailable) return

  try {
    await writer(formatMessage(message, context))
  } catch (error) {
    pluginAvailable = false
    if (isDev) {
      console.debug("Tauri logging plugin rejected a log record.", error)
    }
  }
}

function formatMessage(message: string, context?: LogContext) {
  if (!context) return message
  return `${message} ${safeJson(context)}`
}

function safeJson(value: unknown) {
  try {
    return JSON.stringify(value, (_key, child) => {
      if (child instanceof Error) {
        return {
          name: child.name,
          message: child.message,
          stack: child.stack,
        }
      }
      return child
    })
  } catch {
    return JSON.stringify({ value: String(value) })
  }
}
