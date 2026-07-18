import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const inputVariants = cva(
  "w-full border font-medium text-foreground placeholder:text-faint transition-[background-color,border-color,box-shadow,color] duration-normal ease-standard focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50 motion-reduce:transition-none",
  {
    variants: {
      variant: {
        default:
          "rounded-md border-line bg-surface-0 focus-visible:border-accent/60 focus-visible:ring-1 focus-visible:ring-accent/40 hover:border-line-strong",

        panel:
          "rounded-md border-line bg-surface-1 focus-visible:border-accent/60 focus-visible:ring-1 focus-visible:ring-accent/40 hover:border-line-strong",

        outline:
          "rounded-md border-line bg-transparent focus-visible:border-accent/70 focus-visible:ring-1 focus-visible:ring-accent/40 hover:border-line-strong",

        ghost:
          "rounded-md border-transparent bg-transparent focus-visible:border-line focus-visible:bg-surface-1 focus-visible:ring-1 focus-visible:ring-accent/40 hover:bg-surface-1",

        underline:
          "rounded-none border-x-0 border-t-0 border-b border-b-line bg-transparent px-0 shadow-none focus-visible:border-b-accent focus-visible:ring-0 focus-visible:ring-offset-0 hover:border-b-line-strong",

        filled:
          "rounded-md border-transparent bg-surface-2 focus-visible:border-accent/50 focus-visible:bg-surface-3 focus-visible:ring-1 focus-visible:ring-accent/40 focus-visible:ring-offset-0 hover:bg-surface-3"
      },
      size: {
        sm: "h-8 px-3 text-xs",
        md: "h-10 px-3.5 text-sm",
        lg: "h-12 px-4 text-base"
      }
    },
    compoundVariants: [
      // underline variant overrides horizontal padding for all sizes
      { variant: "underline", size: "sm", class: "px-0" },
      { variant: "underline", size: "md", class: "px-0" },
      { variant: "underline", size: "lg", class: "px-0" }
    ],
    defaultVariants: {
      variant: "default",
      size: "md"
    }
  }
)

export type InputProps = Omit<React.InputHTMLAttributes<HTMLInputElement>, "size"> &
  VariantProps<typeof inputVariants>

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, variant, size, type = "text", ...props }, ref) => {
    return (
      <input
        ref={ref}
        type={type}
        className={cn(inputVariants({ variant, size }), className)}
        {...props}
      />
    )
  }
)

Input.displayName = "Input"
