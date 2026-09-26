import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ConsoleProviderQuotaWindow } from "../../api/contracts";
import {
  formatQuotaReset,
  ProviderQuotaPanel,
  quotaWindowRemainingRatio,
} from "./ProviderAccountCard";

const t = (zh: string, _en: string) => zh;

function quotaWindow(
  overrides: Partial<ConsoleProviderQuotaWindow> = {},
): ConsoleProviderQuotaWindow {
  return {
    key: "daily",
    label: "Daily",
    usedPercent: null,
    remainingRatio: null,
    limitWindowSeconds: null,
    resetAt: null,
    resetAfterSeconds: null,
    ...overrides,
  };
}

describe("account card quota presentation", () => {
  it("normalizes explicit and derived remaining ratios without leaking invalid values", () => {
    expect(quotaWindowRemainingRatio(quotaWindow({ remainingRatio: 1.5 }))).toBe(1);
    expect(quotaWindowRemainingRatio(quotaWindow({ remainingRatio: -0.5 }))).toBe(0);
    expect(quotaWindowRemainingRatio(quotaWindow({ usedPercent: 25 }))).toBe(0.75);
    expect(quotaWindowRemainingRatio(quotaWindow({ remainingRatio: Number.NaN }))).toBeNull();
  });

  it("uses a non-breaking placeholder for missing resets and formats valid resets", () => {
    expect(formatQuotaReset(null, t)).toBe("\u00a0");
    expect(formatQuotaReset("not-a-date", t)).toBe("\u00a0");
    expect(formatQuotaReset("2026-08-20T00:00:00Z", t)).toMatch(/ 重置$/);
  });

  it("renders at most two windows with account counts and explicit USD balance", () => {
    render(
      <ProviderQuotaPanel
        t={t}
        scope="provider"
        remainingUsd={4.25}
        windows={[
          { key: "daily", label: "Daily", remainingRatio: 0.75, resetAt: null, accountCount: 2 },
          { key: "weekly", label: "Weekly", remainingRatio: 0.5, resetAt: null },
          { key: "monthly", label: "Monthly", remainingRatio: 0.25, resetAt: null },
        ]}
      />,
    );

    expect(screen.getAllByText(/Daily|Weekly/)).toHaveLength(2);
    expect(screen.queryByText("Monthly")).not.toBeInTheDocument();
    expect(screen.getByText("75%")).toBeInTheDocument();
    expect(screen.getByText("50%")).toBeInTheDocument();
    expect(screen.getByText("≈$4.25")).toBeInTheDocument();
    expect(screen.getByText(/2 个账号/)).toBeInTheDocument();
  });
});
