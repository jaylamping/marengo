import * as React from "react"
import { Slot } from "@radix-ui/react-slot"

type RenderElement = React.ReactElement | undefined

export function mergeProps(...sources: Record<string, unknown>[]): Record<string, unknown> {
  return Object.assign({}, ...sources)
}

function stateToDataAttributes(state?: Record<string, unknown>): Record<string, string> {
  if (!state) return {}
  const attrs: Record<string, string> = {}
  for (const [key, value] of Object.entries(state)) {
    if (value === undefined || value === null) continue
    if (key === "slot") {
      attrs["data-slot"] = String(value)
      continue
    }
    if (typeof value === "boolean") {
      if (value) attrs[`data-${key}`] = ""
      continue
    }
    attrs[`data-${key}`] = String(value)
  }
  return attrs
}

export type UseRenderOptions<T extends React.ElementType> = {
  defaultTagName: T
  props: Record<string, unknown> & { render?: RenderElement }
  render?: RenderElement
  state?: Record<string, unknown>
}

export function useRender<T extends React.ElementType>({
  defaultTagName,
  props,
  render,
  state,
}: UseRenderOptions<T>): React.ReactElement {
  const { render: renderFromProps, children, ...restProps } = props
  const effectiveRender = render ?? renderFromProps
  const dataAttributes = stateToDataAttributes(state)
  const mergedProps = {
    ...restProps,
    ...dataAttributes,
    ...(children !== undefined ? { children } : {}),
  }

  if (effectiveRender) {
    return <Slot {...mergedProps}>{effectiveRender}</Slot>
  }

  return React.createElement(defaultTagName, mergedProps)
}

export namespace useRender {
  export type ComponentProps<T extends React.ElementType> = React.ComponentProps<T> & {
    render?: RenderElement
  }
}
