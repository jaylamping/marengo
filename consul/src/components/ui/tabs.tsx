import * as React from "react"
import * as TabsPrimitive from "@radix-ui/react-tabs"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const tabsListVariants = cva(
  "inline-flex items-center rounded-md border p-1 transition-[background-color,border-color,box-shadow] duration-fast ease-standard motion-reduce:transition-none",
  {
    variants: {
      variant: {
        default: "border-line bg-surface-0",
        panel: "border-line bg-surface-1",
        outline: "bg-transparent border border-line",
        ghost: "bg-transparent border border-transparent",
      },
      size: {
        sm: "h-8 gap-1",
        md: "h-10 gap-1.5",
        lg: "h-11 gap-2"
      }
    },
    defaultVariants: {
      variant: "default",
      size: "md"
    }
  }
)

const tabsTriggerVariants = cva(
  "inline-flex items-center justify-center whitespace-nowrap rounded-sm font-mono font-medium uppercase tracking-[0.08em] transition-[background-color,color,border-color,box-shadow] duration-fast ease-standard focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent/50 disabled:pointer-events-none disabled:opacity-50 motion-reduce:transition-none",
  {
    variants: {
      variant: {
        default:
          "text-muted-foreground data-[state=active]:bg-surface-2 data-[state=active]:text-foreground",
        panel:
          "text-muted-foreground data-[state=active]:bg-surface-3 data-[state=active]:text-foreground",
        outline:
          "text-muted-foreground border border-transparent data-[state=active]:border-line data-[state=active]:bg-surface-1 data-[state=active]:text-foreground",
        ghost:
          "text-muted-foreground data-[state=active]:bg-surface-2 data-[state=active]:text-foreground",
      },
      size: {
        sm: "h-6 px-2 text-[0.625rem]",
        md: "h-8 px-3 text-xs",
        lg: "h-9 px-4 text-sm"
      }
    },
    defaultVariants: {
      variant: "default",
      size: "md"
    }
  }
)

const tabsContentVariants = cva(
  "mt-2 rounded-md border text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent/50",
  {
    variants: {
      variant: {
        default: "bg-surface-1 border-line",
        panel: "bg-surface-1 border-line",
        outline: "bg-transparent border-line",
        ghost: "bg-transparent border-transparent",
      },
      size: {
        sm: "p-2 text-xs",
        md: "p-3 text-sm",
        lg: "p-4 text-base"
      }
    },
    defaultVariants: {
      variant: "default",
      size: "md"
    }
  }
)

export type TabsProps = React.ComponentPropsWithoutRef<typeof TabsPrimitive.Root>

export const Tabs = TabsPrimitive.Root

export type TabsListProps = React.ComponentPropsWithoutRef<typeof TabsPrimitive.List> &
  VariantProps<typeof tabsListVariants>

export const TabsList = React.forwardRef<React.ComponentRef<typeof TabsPrimitive.List>, TabsListProps>(
  ({ className, variant, size, ...props }, ref) => (
    <TabsPrimitive.List
      ref={ref}
      className={cn(tabsListVariants({ variant, size }), className)}
      {...props}
    />
  )
)

TabsList.displayName = "TabsList"

export type TabsTriggerProps = React.ComponentPropsWithoutRef<typeof TabsPrimitive.Trigger> &
  VariantProps<typeof tabsTriggerVariants>

export const TabsTrigger = React.forwardRef<
  React.ComponentRef<typeof TabsPrimitive.Trigger>,
  TabsTriggerProps
>(({ className, variant, size, ...props }, ref) => (
  <TabsPrimitive.Trigger
    ref={ref}
    className={cn(tabsTriggerVariants({ variant, size }), className)}
    {...props}
  />
))

TabsTrigger.displayName = "TabsTrigger"

export type TabsContentProps = React.ComponentPropsWithoutRef<typeof TabsPrimitive.Content> &
  VariantProps<typeof tabsContentVariants>

export const TabsContent = React.forwardRef<
  React.ComponentRef<typeof TabsPrimitive.Content>,
  TabsContentProps
>(({ className, variant, size, ...props }, ref) => (
  <TabsPrimitive.Content
    ref={ref}
    className={cn(tabsContentVariants({ variant, size }), className)}
    {...props}
  />
))

TabsContent.displayName = "TabsContent"

export { tabsListVariants }
