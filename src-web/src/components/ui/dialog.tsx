import * as React from "react"
import { cn } from "@/lib/utils"

export function Dialog({ children }: { children: React.ReactNode }) {
  return <>{children}</>
}

export const DialogContent = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("rounded-lg border bg-card p-5 shadow-lg", className)} {...props} />
  ),
)
DialogContent.displayName = "DialogContent"

export const DialogHeader = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => <div ref={ref} className={cn("space-y-1", className)} {...props} />,
)
DialogHeader.displayName = "DialogHeader"

export const DialogTitle = React.forwardRef<HTMLHeadingElement, React.HTMLAttributes<HTMLHeadingElement>>(
  ({ className, ...props }, ref) => <h2 ref={ref} className={cn("text-lg font-semibold", className)} {...props} />,
)
DialogTitle.displayName = "DialogTitle"
