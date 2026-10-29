import { describe, expect, it } from "vitest";
import { aggregateProviderMetrics, aggregateSuccessWindows } from "./providerCardMetrics";
import { formatAvailabilityRange, recentAvailabilityWindows } from "./availabilityWindows";

const window = (label: string, success = 1, requests = 1) => ({ label, success, requests });

describe("half-hour availability windows", () => {
  it("fills missing half hours without treating no traffic as failure or copying counts", () => {
    const windows = recentAvailabilityWindows(aggregateSuccessWindows([{ successWindows: [
      window("2026-10-25T23:30:00Z", 2, 3), window("2026-10-26T00:30:00Z", 1, 2),
    ] }]));
    expect(windows).toEqual([
      { ...window("2026-10-25T23:30:00Z", 2, 3), rate: 2 / 3 },
      { ...window("2026-10-26T00:00:00Z", 0, 0), rate: null },
      { ...window("2026-10-26T00:30:00Z", 1, 2), rate: 0.5 },
    ]);
  });

  it("bounds sparse history to eight contiguous half hours and keeps weighted totals", () => {
    const metrics = aggregateProviderMetrics([{ successWindows: [
      window("2026-10-24T00:00:00Z", 100, 100), window("2026-10-26T04:00:00Z", 1, 2),
      window("2026-10-26T04:30:00Z", 2, 2),
    ] }]);
    expect(metrics.successWindows).toHaveLength(8);
    expect(metrics.successWindows[0].label).toBe("2026-10-26T01:00:00Z");
    expect(metrics.successRequestCount).toBe(4);
    expect(metrics.successRate).toBe(0.75);
  });

  it("does not invent traffic when no windows exist and tolerates legacy time-only labels", () => {
    expect(recentAvailabilityWindows([])).toEqual([]);
    expect(recentAvailabilityWindows(aggregateSuccessWindows([{ successWindows: [
      window("10:00"), window("11:00"),
    ] }])).map((entry) => entry.label)).toEqual(["10:00", "10:30", "11:00"]);
    expect(formatAvailabilityRange("23:30")).toBe("23:30–00:00");
    expect(formatAvailabilityRange("10:00")).toBe("10:00–10:30");
  });

  it("formats timestamp ranges in local time and never drops the half hour at midnight", () => {
    const label = "2026-10-25T23:30:00Z";
    const clock = (ms: number) => {
      const date = new Date(ms);
      return `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
    };
    const start = Date.parse(label);
    expect(formatAvailabilityRange(label)).toBe(`${clock(start)}–${clock(start + 1_800_000)}`);
  });
});
