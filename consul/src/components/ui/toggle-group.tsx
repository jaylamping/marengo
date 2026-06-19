import * as React from "react"
import * as ToggleGroupPrimitive from "@radix-ui/react-toggle-group"
import { type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"
import { toggleVariants } from "@/components/ui/toggle"

const ToggleGroupContext = React.createContext<
  VariantProps<typeof toggleVariants> & {
    spacing?: number
    orientation?: "horizontal" | "vertical"
  }
>({
  size: "default",
  variant: "default",
  spacing: 2,
  orientation: "horizontal",
})

type ToggleGroupProps = Omit<
  React.ComponentPropsWithoutRef<typeof ToggleGroupPrimitive.Root>,
  "type" | "value" | "defaultValue" | "onValueChange"
> &
  VariantProps<typeof toggleVariants> & {
    spacing?: number
    orientation?: "horizontal" | "vertical"
    multiple?: boolean
    value?: string[]
    defaultValue?: string[]
    onValueChange?: (value: string[]) => void
  }

function ToggleGroup({
  className,
  variant,
  size,
  spacing = 2,
  orientation = "horizontal",
  multiple = true,
  value,
  defaultValue,
  onValueChange,
  children,
  ...props
}: ToggleGroupProps) {
  const sharedClassName = cn(
    "group/toggle-group flex w-fit flex-row items-center gap-[--spacing(var(--gap))] rounded-md data-[size=sm]:rounded-[min(var(--radius-md),8px)] data-[orientation=vertical]:flex-col data-[orientation=vertical]:items-stretch",
    className
  )
  const sharedProps = {
    "data-slot": "toggle-group",
    "data-variant": variant,
    "data-size": size,
    "data-spacing": spacing,
    "data-orientation": orientation,
    style: { "--gap": spacing } as React.CSSProperties,
    className: sharedClassName,
    ...props,
  }

  const provider = (
    <ToggleGroupContext.Provider value={{ variant, size, spacing, orientation }}>
      {children}
    </ToggleGroupContext.Provider>
  )

  if (multiple === false) {
    return (
      <ToggleGroupPrimitive.Root
        {...sharedProps}
        type="single"
        value={value?.[0]}
        defaultValue={defaultValue?.[0]}
        onValueChange={(next) => onValueChange?.(next ? [next] : [])}
      >
        {provider}
      </ToggleGroupPrimitive.Root>
    )
  }

  return (
    <ToggleGroupPrimitive.Root
      {...sharedProps}
      type="multiple"
      value={value}
      defaultValue={defaultValue}
      onValueChange={onValueChange}
    >
      {provider}
    </ToggleGroupPrimitive.Root>
  )
}

function ToggleGroupItem({
  className,
  children,
  variant = "default",
  size = "default",
  ...props
}: React.ComponentPropsWithoutRef<typeof ToggleGroupPrimitive.Item> &
  VariantProps<typeof toggleVariants>) {
  const context = React.useContext(ToggleGroupContext)

  return (
    <ToggleGroupPrimitive.Item
      data-slot="toggle-group-item"
      data-variant={context.variant || variant}
      data-size={context.size || size}
      data-spacing={context.spacing}
      className={cn(
        "shrink-0 group-data-[spacing=0]/toggle-group:rounded-none group-data-[spacing=0]/toggle-group:px-2 focus:z-10 focus-visible:z-10 group-data-[spacing=0]/toggle-group:has-data-[icon=inline-end]:pr-1.5 group-data-[spacing=0]/toggle-group:has-data-[icon=inline-start]:pl-1.5 group-data-[orientation=horizontal]/toggle-group:data-[spacing=0]:first:rounded-l-md group-data-[orientation=vertical]/toggle-group:data-[spacing=0]:first:rounded-t-md group-data-[orientation=horizontal]/toggle-group:data-[spacing=0]:last:rounded-r-md group-data-[orientation=vertical]/toggle-group:data-[spacing=0]:last:rounded-b-md group-data-[orientation=horizontal]/toggle-group:data-[spacing=0]:data-[variant=outline]:border-l-0 group-data-[orientation=vertical]/toggle-group:data-[spacing=0]:data-[variant=outline]:border-t-0 group-data-[orientation=horizontal]/toggle-group:data-[spacing=0]:data-[variant=outline]:first:border-l group-data-[orientation=vertical]/toggle-group:data-[spacing=0]:data-[variant=outline]:first:border-t",
        toggleVariants({
          variant: context.variant || variant,
          size: context.size || size,
        }),
        className
      )}
      {...props}
    >
      {children}
    </ToggleGroupPrimitive.Item>
  )
}

export { ToggleGroup, ToggleGroupItem }
