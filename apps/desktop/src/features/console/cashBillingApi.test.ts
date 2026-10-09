import { describe, expect, it, vi } from "vitest";
import {
  accountBillingSchema,
  cashBillingApi,
  cashLedgerSchema,
} from "./cashBillingApi";

describe("cash billing transport", () => {
  it("encodes IDs and patches only the selected account, including explicit default removal", async () => {
    const request = vi
      .fn()
      .mockResolvedValue({ providerAccount: { id: "p/a" } });
    const api = cashBillingApi({ request });
    const signal = new AbortController().signal;
    await api.saveMultiplier("admin", "p/a", "account-a", null, signal);
    expect(request).toHaveBeenCalledWith(
      "/v1/internal/gateway/provider-accounts/p%2Fa/model-pricing",
      expect.anything(),
      {
        method: "POST",
        managementToken: "admin",
        signal,
        body: { accountBillingMultipliers: { "account-a": null } },
      },
    );
    await api.ledger("admin", "key/a", signal);
    expect(request.mock.calls[1][0]).toBe(
      "/v1/internal/gateway/access/keys/key%2Fa/cash-ledger",
    );
  });
  it("rejects imprecise amounts, unknown states and oversized responses", () => {
    const row = {
      requestId: "r",
      accessKeyId: "old-key",
      createdAt: "2026-10-08T00:00:00Z",
      status: "unresolved",
      currency: "USD",
      reservedMicros: 1,
      amountMicros: null,
      quotes: [],
      settledQuote: null,
      reason: "usage_unavailable",
      usage: null,
    };
    expect(cashLedgerSchema.parse([row])[0].amountMicros).toBeNull();
    expect(
      cashLedgerSchema.safeParse([
        { ...row, reservedMicros: Number.MAX_SAFE_INTEGER + 1 },
      ]).success,
    ).toBe(false);
    expect(
      cashLedgerSchema.safeParse([{ ...row, status: "unknown" }]).success,
    ).toBe(false);
    expect(cashLedgerSchema.safeParse(Array(101).fill(row)).success).toBe(
      false,
    );
    expect(
      accountBillingSchema.safeParse({
        accountBillingMultipliers: { a: "1000000.000001" },
      }).success,
    ).toBe(false);
    expect(
      accountBillingSchema.parse({
        accountBillingMultipliers: { a: "0.000000" },
      }).accountBillingMultipliers.a,
    ).toBe("0.000000");
  });
});
