// @vitest-environment node
import { describe, it, expect } from "vitest"
import pkg from "../../../../package.json"

const uiSources = import.meta.glob(
  ["../**/*.{ts,tsx}", "!../**/__tests__/**"],
  { query: "?raw", import: "default", eager: true }
) as Record<string, string>

describe("Base UI lint gate (PR8)", () => {
  it("has zero @base-ui/react imports under ui/*", () => {
    const offenders = Object.entries(uiSources).flatMap(([file, source]) => {
      const matches = source.match(/from\s+["']@base-ui\/react/g)
      return matches ? [`${file}: ${matches.length}`] : []
    })

    expect(offenders, offenders.join("\n")).toEqual([])
  })

  it("does not declare @base-ui/react in package.json dependencies", () => {
    expect(JSON.stringify(pkg)).not.toMatch(/"@base-ui\/react"/)
  })
})
