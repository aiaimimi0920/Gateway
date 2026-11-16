import { expect, it, vi } from "vitest";
import { createGatewayApiClient, type GatewayApiClient } from "../client";
import { createBrowserHost } from "../../platform/browserHost";
import { createFreebuffPurchaseApi, freebuffPendingPurchases } from "./freebuff-purchases";
import { pendingPurchase, pendingResponse, purchaseBinding } from "./freebuff-purchases.fixtures";

function fixture(value: unknown = pendingResponse()) {
  const request = vi.fn(async (_path, _schema, _options) => value);
  return { request, api: createFreebuffPurchaseApi({ request } as GatewayApiClient, "owner"), signal: new AbortController().signal };
}
it("reads only the saved account and sends only the original host-issued IDs for an explicit decision", async () => {
  const { request, api, signal } = fixture(); const rows = await api.read(purchaseBinding, "grant", signal);
  expect(rows).toEqual([pendingPurchase()]);
  expect(request).toHaveBeenCalledExactlyOnceWith("/v1/internal/gateway/console/freebuff-pending-purchases", expect.anything(), {
    method: "POST", managementToken: "owner", secretGrant: "grant", signal, body: purchaseBinding,
  });
  request.mockResolvedValue({ delivered: true });
  for (const decision of ["approve", "cancel"] as const) {
    await api.decide(purchaseBinding, rows[0], decision, "grant", signal);
    expect(request.mock.calls.at(-1)![2].body).toEqual({ ...purchaseBinding, pendingId: rows[0].pendingId, quoteId: rows[0].quoteId, decision });
  }
});
it("uses the existing same-origin secret header, no-store and no-redirect transport", async () => {
  const fetchImplementation = vi.fn().mockResolvedValue(new Response(JSON.stringify(pendingResponse()), { status: 200 }));
  const api = createFreebuffPurchaseApi(createGatewayApiClient({ host: createBrowserHost(), fetchImplementation }), "owner");
  await api.read(purchaseBinding, "grant", new AbortController().signal);
  const [url, options] = fetchImplementation.mock.calls[0];
  expect(url).toBe("https://gateway.test/v1/internal/gateway/console/freebuff-pending-purchases");
  expect(options).toMatchObject({ method: "POST", redirect: "error", cache: "no-store", credentials: "omit" });
  expect(options.headers.get("x-management-token")).toBe("owner"); expect(options.headers.get("x-secret-grant")).toBe("grant");
  expect(JSON.parse(options.body)).toEqual(purchaseBinding);
});
it.each(["revision", "providerId", "credentialId"])("rejects a foreign saved-account identity: %s", async field => {
  const { api, signal } = fixture({ ...pendingResponse(), [field]: "foreign" });
  await expect(api.read(purchaseBinding, "grant", signal)).rejects.toThrow();
});
it.each([
  { pendingId: "not-a-uuid" }, { quoteId: "801fe639-14e0-12a8-9c8c-3c751921724a" }, { requestId: "bad" },
  { callerFingerprint: "a".repeat(63) }, { requestSha256: "B".repeat(64) }, { authToken: "secret" },
  { quote: { ...pendingPurchase().quote, walletSpend: -1 } }, { quote: { ...pendingPurchase().quote, claimEarned: "true" } },
  { quote: { ...pendingPurchase().quote, modelKey: "bad\nmodel" } },
  ...[null, -1, Infinity, NaN].map(sessionPrice => ({ quote: { ...pendingPurchase().quote, meter: { ...pendingPurchase().quote.meter, sessionPrice } } })),
  { quote: { ...pendingPurchase().quote, meter: { ...pendingPurchase().quote.meter, deviceKey: "secret" } } },
])("rejects malformed or secret-bearing purchase views case %#: %j", async patch => {
  const { api, signal } = fixture({ ...pendingResponse(), pending: [{ ...pendingPurchase(), ...patch }] });
  await expect(api.read(purchaseBinding, "grant", signal)).rejects.toThrow();
});
it("bounds the queue and rejects ambiguous duplicate request/quote/pending identities", () => {
  const row = pendingPurchase();
  expect(freebuffPendingPurchases.safeParse(pendingResponse(Array(17).fill(row))).success).toBe(false);
  for (const field of ["pendingId", "quoteId", "requestId"] as const) {
    const other = { ...row, pendingId: "00000000-0000-4000-8000-000000000001", quoteId: "00000000-0000-4000-8000-000000000002",
      requestId: "00000000-0000-4000-8000-000000000003", [field]: row[field] };
    expect(freebuffPendingPurchases.safeParse(pendingResponse([row, other])).success).toBe(false);
  }
});
it("accepts a free quote and unknown optional flags without inventing values", () => {
  const row = pendingPurchase(); row.quote.meter = { balance: 0, dailyRemaining: 0, walletBalance: 0, sessionPrice: 0 };
  row.quote.walletSpend = 0;
  expect(freebuffPendingPurchases.parse(pendingResponse([row])).pending[0].quote.meter).toEqual(row.quote.meter);
});
it.each([{ delivered: false }, { delivered: true, purchased: true }, {}, null])("does not confuse malformed delivery with purchase success: %j", async value => {
  const { api, signal, request } = fixture(value);
  await expect(api.decide(purchaseBinding, pendingPurchase(), "approve", "grant", signal)).rejects.toThrow();
  expect(request).toHaveBeenCalledOnce();
});
