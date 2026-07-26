import { describe, it, expect } from "vitest"
import { CRON_PRESETS } from "../pages/Settings"

describe("Settings CRON_PRESETS", () => {
  it("contains the Daily at 6:05, 10:05, 18:05 preset", () => {
    const preset = CRON_PRESETS.find(
      (p) => p.label === "Daily at 6:05, 10:05, 18:05",
    )
    expect(preset).toBeDefined()
    expect(preset!.value).toBe("5 6,10,18 * * *")
  })

  it("has unique labels", () => {
    const labels = CRON_PRESETS.map((p) => p.label)
    expect(new Set(labels).size).toBe(labels.length)
  })

  it("has unique values", () => {
    const values = CRON_PRESETS.map((p) => p.value)
    expect(new Set(values).size).toBe(values.length)
  })

  it("all cron values are valid five-field expressions", () => {
    const fiveField =
      /^(\*|\d+|\d+(,\d+)*|\d+-\d+|\*\/\d+)( (\*|\d+|\d+(,\d+)*|\d+-\d+|\*\/\d+)){4}$/
    for (const preset of CRON_PRESETS) {
      expect(preset.value).toMatch(fiveField)
    }
  })

  it("includes the 6:05 preset in the correct position", () => {
    const idx = CRON_PRESETS.findIndex((p) => p.label.includes("6:05"))
    expect(idx).toBeGreaterThanOrEqual(0)
    // Should be after "Daily at 06:00, 10:00, 12:00"
    const prevIdx = CRON_PRESETS.findIndex((p) => p.label.includes("12:00"))
    expect(idx).toBeGreaterThan(prevIdx)
  })

  it("contains the Daily at 10:05, 18:05 preset", () => {
    const preset = CRON_PRESETS.find((p) => p.label === "Daily at 10:05, 18:05")
    expect(preset).toBeDefined()
    expect(preset!.value).toBe("5 10,18 * * *")
  })
})
