import { z } from "zod";
import type { GatewayApiClient } from "../client";

const amount = z.number().finite().nonnegative();
const id = z.uuid().refine(value => value === value.toLowerCase() && value[14] === "4");
const digest = z.string().regex(/^[a-f0-9]{64}$/);
// Informational, bounded views only. Neither this DTO nor a delivered decision is a purchase receipt.
export const freebuffPendingPurchase = z.object({
  pendingId: id, quoteId: id, requestId: z.uuid(), callerFingerprint: digest, requestSha256: digest,
  quote: z.object({
    modelKey: z.string().min(1).max(1024).refine(value => !/[\u0000-\u001f\u007f]/.test(value)),
    meter: z.object({ balance: amount, dailyRemaining: amount, walletBalance: amount, sessionPrice: amount,
      claimableGrant: amount.optional(), soloDiscount: amount.optional(), quotaExempt: z.boolean().optional(),
      spendWithoutAsking: z.boolean().optional() }).strict(),
    walletSpend: amount, claimEarned: z.boolean(),
  }).strict(),
}).strict();
export const freebuffPendingPurchases = z.object({
  revision: z.string().min(1), providerId: z.string().min(1), credentialId: z.string().min(1),
  pending: z.array(freebuffPendingPurchase).max(16),
}).strict().refine(value => ["pendingId", "quoteId", "requestId"].every(field =>
  new Set(value.pending.map(row => row[field as "pendingId" | "quoteId" | "requestId"])).size === value.pending.length));
const delivered = z.object({ delivered: z.literal(true) }).strict();
export type FreebuffPendingPurchase = z.infer<typeof freebuffPendingPurchase>;
export type FreebuffPurchaseBinding = { providerId: string; credentialId: string; expectedRevision: string };
export type FreebuffPurchaseDecision = "approve" | "cancel";
const endpoint = "/v1/internal/gateway/console/freebuff-pending-purchases";

export function createFreebuffPurchaseApi(client: GatewayApiClient, managementToken: string) {
  return {
    async read(binding: FreebuffPurchaseBinding, secretGrant: string, signal: AbortSignal) {
      const value = freebuffPendingPurchases.parse(await client.request(endpoint, freebuffPendingPurchases, {
        method: "POST", managementToken, secretGrant, signal, body: binding,
      }));
      if (value.revision !== binding.expectedRevision || value.providerId !== binding.providerId || value.credentialId !== binding.credentialId) {
        throw new Error("Pending purchases did not match the saved account.");
      }
      return value.pending;
    },
    async decide(binding: FreebuffPurchaseBinding, row: FreebuffPendingPurchase, decision: FreebuffPurchaseDecision, secretGrant: string, signal: AbortSignal) {
      delivered.parse(await client.request(`${endpoint}/decision`, delivered, {
        method: "POST", managementToken, secretGrant, signal,
        body: { ...binding, pendingId: row.pendingId, quoteId: row.quoteId, decision },
      }));
    },
  };
}
