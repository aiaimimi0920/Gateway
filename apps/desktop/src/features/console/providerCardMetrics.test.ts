import { describe, expect, it } from "vitest";

import {
  aggregateProviderQuotaWindows,
  aggregateQuotaRemainingUsd,
  aggregateProviderConcurrency,
  aggregateProviderCosts,
  aggregateProviderRequests,
  aggregateSuccessWindows,
  buildProviderAvailabilityCells,
  buildProviderPoolSegments,
  classifyAccountPoolState,
} from "./providerCardMetrics";

describe("providerCardMetrics", () => {
  it("builds the requested 78/25/37/60 pool split", () => {
    const accounts = [
      ...Array.from({ length: 78 }, () => ({ statusLabel: "正常", dispatchEnabled: true })),
      ...Array.from({ length: 25 }, () => ({ statusLabel: "限流等待恢复", dispatchEnabled: true })),
      ...Array.from({ length: 37 }, () => ({ statusLabel: "失效", dispatchEnabled: false })),
    ];
    const result = buildProviderPoolSegments(accounts, 200);
    expect(result.available).toBe(78);
    expect(result.rateLimited).toBe(25);
    expect(result.invalid).toBe(37);
    expect(result.remaining).toBe(60);
    expect(result.segments.map((segment) => segment.count)).toEqual([78, 25, 37, 60]);
    expect(result.segments.reduce((sum, segment) => sum + segment.ratio, 0)).toBeCloseTo(1);
  });

  it("normalizes observed states when they exceed the target and handles zero targets", () => {
    const accounts = [
      { statusLabel: "正常", dispatchEnabled: true },
      { statusLabel: "限流", dispatchEnabled: true },
      { statusLabel: "失效", dispatchEnabled: false },
    ];
    const over = buildProviderPoolSegments(accounts, 1);
    expect(over.remaining).toBe(0);
    expect(over.segments.reduce((sum, segment) => sum + segment.ratio, 0)).toBeCloseTo(1);
    const zero = buildProviderPoolSegments([], 0);
    expect(zero.segments.every((segment) => segment.ratio === 0)).toBe(true);
  });

  it("does not infer unknown states from disabled accounts", () => {
    expect(classifyAccountPoolState({ statusLabel: "暂停", dispatchEnabled: false })).toBe("unknown");
  });

  it("keeps unavailable monitoring data unavailable", () => {
    expect(aggregateProviderConcurrency([{ usageWindowBadges: [] }])).toBeNull();
    expect(aggregateProviderCosts([{ usageWindowBadges: [] }])).toEqual({ upstream: null, user: null });
    expect(aggregateProviderRequests([{ usageWindowBadges: [] }])).toBeNull();
    expect(aggregateSuccessWindows([{ successWindows: null }])).toEqual([]);
  });

  it("aggregates explicit concurrency, costs and five-minute windows", () => {
    expect(
      aggregateProviderConcurrency([
        { concurrencyUsed: 2, concurrencyTotal: 5 },
        { concurrencyUsed: 1, concurrencyTotal: 4 },
      ]),
    ).toEqual({ used: 3, total: 9 });
    expect(
      aggregateProviderCosts([
        { upstreamCost: 1.25, userCost: 2.5 },
        { usageWindowBadges: ["A $0.75", "U $1.50"] },
      ]),
    ).toEqual({ upstream: 2, user: 4 });
    expect(
      aggregateProviderRequests([
        { usageWindowBadges: ["1,200 req", "A $1.25"] },
        { usageWindowBadges: ["80 req", "invalid requests"] },
      ]),
    ).toBe(1280);
    expect(
      aggregateSuccessWindows([
        { successWindows: [{ label: "10:00", success: 3, requests: 4 }] },
        { successWindows: [{ label: "10:00", success: 1, requests: 2 }] },
      ]),
    ).toEqual([{ label: "10:00", success: 4, requests: 6, rate: 4 / 6 }]);
  });

  it("reports live concurrency without a ceiling when no account declares a limit", () => {
    // A card must still show the live number when the route document sets no
    // concurrency limit, because "0 / —" is a real answer while "—" reads as
    // monitoring being down.
    expect(
      aggregateProviderConcurrency([
        { concurrencyUsed: 0, concurrencyTotal: null },
        { concurrencyUsed: 2 },
      ]),
    ).toEqual({ used: 2, total: null });
    // A ceiling below the live count is stale, so the live count wins rather
    // than rendering an impossible 3 / 1.
    expect(aggregateProviderConcurrency([{ concurrencyUsed: 3, concurrencyTotal: 1 }])).toEqual({
      used: 3,
      total: 3,
    });
  });

  it("prefers explicit request counts over the legacy badge text and keeps a real zero", () => {
    expect(
      aggregateProviderRequests([
        { requestCount: 7, usageWindowBadges: ["1,200 req"] },
        { usageWindowBadges: ["80 req"] },
      ]),
    ).toBe(87);
    // Zero requests is data, not missing data, so the card shows 0 instead of —.
    expect(aggregateProviderRequests([{ requestCount: 0 }])).toBe(0);
  });

  it("orders recent windows chronologically before the card keeps the latest four", () => {
    expect(
      aggregateSuccessWindows([
        { successWindows: [{ label: "10:05", success: 5, requests: 5 }] },
        { successWindows: [{ label: "09:55", success: 4, requests: 5 }] },
        { successWindows: [{ label: "10:00", success: 3, requests: 4 }] },
        { successWindows: [{ label: "10:10", success: 2, requests: 3 }] },
      ]).map((window) => window.label),
    ).toEqual(["09:55", "10:00", "10:05", "10:10"]);
  });

  it("builds compact availability cells from provider-level windows", () => {
    const windows = aggregateSuccessWindows([
      {
        successWindows: [
          { label: "10:00", success: 3, requests: 4 },
          { label: "10:05", success: 5, requests: 8 },
          { label: "10:10", success: 0, requests: 0 },
        ],
      },
    ]);
    const cells = buildProviderAvailabilityCells(windows, 4);

    expect(cells).toHaveLength(12);
    expect(cells.filter((cell) => cell.state === "success")).toHaveLength(5);
    expect(cells.filter((cell) => cell.state === "mixed")).toHaveLength(1);
    expect(cells.filter((cell) => cell.state === "failure")).toHaveLength(2);
    expect(cells.filter((cell) => cell.state === "empty")).toHaveLength(4);
    expect(cells.filter((cell) => cell.windowIndex === 1)).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ windowLabel: "10:05", success: 5, requests: 8 }),
      ]),
    );
  });

  it("aggregates success windows across provider accounts without a model split", () => {
    const modelAccounts = [
      {
        modelId: "gpt-5.6",
        successWindows: [{ label: "10:00", success: 8, requests: 10 }],
      },
      {
        modelId: "gpt-5.4",
        successWindows: [{ label: "10:00", success: 7, requests: 10 }],
      },
    ];

    expect(aggregateSuccessWindows(modelAccounts)).toEqual([
      { label: "10:00", success: 15, requests: 20, rate: 0.75 },
    ]);
  });

  it("aggregates quota windows without inventing missing values", () => {
    const accounts = [
      {
        quotaRemainingUsd: 3.25,
        quota: {
          windows: [
            {
              key: "primary",
              label: "5 小时",
              remainingRatio: 0.8,
              limitWindowSeconds: 18_000,
              resetAt: "2026-08-19T10:00:00Z",
            },
          ],
        },
      },
      {
        quotaRemainingUsd: null,
        quota: {
          windows: [
            {
              key: "primary",
              label: "5 小时",
              usedPercent: 40,
              limitWindowSeconds: 18_000,
              resetAt: "2026-08-19T09:00:00Z",
            },
          ],
        },
      },
    ];

    expect(aggregateProviderQuotaWindows(accounts)).toEqual([
      {
        key: "primary",
        label: "5 小时",
        remainingRatio: 0.7,
        accountCount: 2,
        resetAt: "2026-08-19T09:00:00Z",
      },
    ]);
    expect(aggregateQuotaRemainingUsd(accounts)).toBe(3.25);
    expect(aggregateQuotaRemainingUsd([{ quotaRemainingUsd: null }])).toBeNull();
  });
});
