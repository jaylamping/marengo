import * as React from "react"
import { cn } from "@/lib/utils"
import { cva, type VariantProps } from "class-variance-authority"

// Simple slider primitives to mimic shadcn/glinui style using native range input.
// This file provides 4 exports: Slider, SliderTrack, SliderRange, SliderThumb

// Base variants to align with existing design tokens (variants used by other UI components)
const sliderTrackVariants = cva(
  "relative w-full flex items-center",
  {
    variants: {
      size: {
        sm: "h-2",
        md: "h-3",
        lg: "h-4",
      },
      variant: {
        default: "",
        glass: "bg-white/5",
      },
    },
    defaultVariants: {
      size: "md",
      variant: "default",
    },
  }
)

const sliderRootVariants = cva("w-full", {
  variants: {
    size: {
      sm: "h-6",
      md: "h-8",
      lg: "h-10",
    },
    variant: {
      default: "",
      glass: "bg-white/5 rounded-md",
    },
  },
  defaultVariants: {
    size: "md",
    variant: "default",
  },
})

export type SliderProps = Omit<React.InputHTMLAttributes<HTMLInputElement>, 'value' | 'defaultValue' | 'onChange'> & {
  value?: number[]
  defaultValue?: number[]
  onValueChange?: (value: number[]) => void
  // expose optional design variants for consumers
  size?: "sm" | "md" | "lg"
  variant?: "default" | "glass"
}

// Slider root that renders a native range input for single-value sliders.
export const Slider = React.forwardRef<HTMLInputElement, SliderProps>(
  ({ value, defaultValue, min = 0, max = 100, step = 1, onValueChange, size = "md", variant = "default", className, style, ...props }, ref) => {
    // Support both controlled (value) and uncontrolled (defaultValue) usage for a single handle.
    const hasValueArray = Array.isArray(value) && value.length > 0
    const minValue = (typeof min === 'number' ? min : Number(min ?? 0)) as number
    const [internalValue, setInternalValue] = React.useState<number>(
      hasValueArray ? value![0] : (defaultValue && defaultValue[0] != null) ? defaultValue![0] : minValue
    )

    // If external value prop changes, reflect it (controlled usage)
    React.useEffect(() => {
      if (hasValueArray) {
        setInternalValue(value![0])
      }
    }, [value])

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const v = Number(e.target.value)
      setInternalValue(v)
      onValueChange?.([v])
    }

    return (
      <div className={cn("w-full", className)} style={style}>
        {/* Wrap with track for API compatibility; actual thumb is provided by native range */}
        <div className={sliderRootVariants({ size: size as "sm" | "md" | "lg", variant: variant as "default" | "glass" })}>
          <input
            ref={ref}
            type="range"
            min={min}
            max={max}
            step={step}
            value={internalValue}
            onChange={handleChange}
            className={cn(
              // basic track styling to resemble glinui radix-like slider
              "w-full appearance-none rounded-lg border-0 bg-transparent",
              "accent-[#4f46e5]", // tailwind purple-500-ish accent for modern look
              size === "sm" ? "h-2" : size === "lg" ? "h-4" : "h-3"
            )}
            {...props}
          />
        </div>
      </div>
    )
  }
)
Slider.displayName = "Slider"

export const SliderTrack = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  (props, ref) => {
    // Simple track wrapper; actual native range handles the thumb visually.
    return <div ref={ref} {...props} className={cn("relative w-full rounded-full bg-gray-200/60", props.className)} />
  }
)
SliderTrack.displayName = "SliderTrack"

export const SliderRange = React.forwardRef<HTMLSpanElement, React.HTMLAttributes<HTMLSpanElement>>((props, ref) => {
  return <span ref={ref} {...props} className={cn("absolute left-0 top-0 h-full bg-gray-700/40 rounded-full", props.className)} />
})
SliderRange.displayName = "SliderRange"

export const SliderThumb = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>((props, ref) => {
  return <div ref={ref} {...props} className={cn("absolute w-4 h-4 rounded-full bg-white shadow", props.className)} />
})
SliderThumb.displayName = "SliderThumb"

export default Slider
