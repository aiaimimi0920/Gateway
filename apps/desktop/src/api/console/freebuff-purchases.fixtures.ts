import type { FreebuffPendingPurchase } from "./freebuff-purchases";
export const purchaseBinding = { providerId: "pool", credentialId: "stable", expectedRevision: "r1" };
export function pendingPurchase(): FreebuffPendingPurchase {
  return { pendingId: "801fe639-14e0-42a8-9c8c-3c751921724a", quoteId: "a756c431-3e72-4018-baff-28cd9a5ea936",
    requestId: "9eae55c0-88ad-4193-9aa4-7e676782d593", callerFingerprint: "a".repeat(64), requestSha256: "b".repeat(64),
    quote: { modelKey: "catalog-model", meter: { balance: 150, dailyRemaining: 30, walletBalance: 120, sessionPrice: 50,
      claimableGrant: 10, soloDiscount: 2, quotaExempt: false, spendWithoutAsking: true }, walletSpend: 20, claimEarned: false } };
}
export function pendingResponse(rows = [pendingPurchase()]) {
  return { revision: purchaseBinding.expectedRevision, providerId: purchaseBinding.providerId, credentialId: purchaseBinding.credentialId, pending: rows };
}
