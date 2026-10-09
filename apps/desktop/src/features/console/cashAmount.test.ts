import { describe, expect, it } from "vitest";
import { cashRemaining, formatCashMicros, parseCashMicros } from "./cashAmount";
import { keyQuotaDraft, keyQuotaInput, validKeyQuota } from "./accessKeyQuota";
import { consoleAccessKeyBalanceSchema } from "../../api/schemas/access";

describe("USD cash quota precision", () => {
  it("parses exact micro-dollars without binary rounding", () => {
    expect(parseCashMicros("2.40")).toBe(2_400_000);
    expect(parseCashMicros("0.000001")).toBe(1);
    expect(parseCashMicros("9007199254.740991")).toBe(Number.MAX_SAFE_INTEGER);
    expect(formatCashMicros(Number.MAX_SAFE_INTEGER)).toBe("9007199254.740991");
    expect(formatCashMicros(-1)).toBe("-0.000001");
  });

  it("rejects unknown precision, negative, scientific and unsafe input", () => {
    for (const value of [
      "",
      "-1",
      "NaN",
      "1e3",
      "0.0000001",
      "9007199254.740992",
      "1.2.3",
    ])
      expect(
        validKeyQuota({ quotaMode: "cash_prepaid", quotaLimit: value }),
      ).toBe(false);
    expect(
      keyQuotaInput({ quotaMode: "cash_prepaid", quotaLimit: "2.4" }),
    ).toEqual({
      mode: "cash_prepaid",
      limit: 2_400_000,
      currency: "USD",
    });
  });

  it("retains pending money separately from spent and preserves old wire contracts", () => {
    const balance = consoleAccessKeyBalanceSchema.parse({
      accessKeyId: "k",
      balanceMode: "cash_prepaid",
      status: "active",
      updatedAt: "2026-10-08",
      cash: {
        currency: "USD",
        totalMicros: 3_000_000,
        spentMicros: 2_400_000,
        reservedMicros: 500_000,
        pendingRequests: 1,
      },
    });
    expect(keyQuotaDraft(balance)).toEqual({
      quotaMode: "cash_prepaid",
      quotaLimit: "3",
    });
    expect(formatCashMicros(cashRemaining(balance.cash!))).toBe("0.1");
    expect(
      consoleAccessKeyBalanceSchema.parse({
        accessKeyId: "old",
        balanceMode: "unlimited",
        status: "active",
        updatedAt: "2026-10-08",
      }).cash,
    ).toBeUndefined();
    expect(
      consoleAccessKeyBalanceSchema.safeParse({
        ...balance,
        cash: { ...balance.cash, spentMicros: 0.5 },
      }).success,
    ).toBe(false);
  });
});
