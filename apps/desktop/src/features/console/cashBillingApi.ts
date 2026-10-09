import { z } from "zod";
import type { GatewayApiClient } from "../../api/client";
import { parseCashMicros } from "./cashAmount";

const integer = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER);
const multiplier = z.union([z.string().max(40), z.number()]).refine((value) => {
  const parsed = parseCashMicros(String(value));
  return parsed !== null && parsed <= 1_000_000_000_000;
});
export const accountBillingSchema = z.object({
  accountBillingMultipliers: z.record(z.string(), multiplier),
});
const quote = z.object({
  providerAccountId: z.string(),
  credentialId: z.string(),
  model: z.string(),
  groupId: z.string().nullable(),
  priceSource: z.string(),
  promptMicrosPer1kTokens: integer,
  completionMicrosPer1kTokens: integer,
  groupMultiplierPpm: integer,
  accountMultiplierPpm: integer,
  cacheTokensSeparate: z.boolean(),
});
export const cashLedgerSchema = z
  .array(
    z.object({
      requestId: z.string(),
      accessKeyId: z.string(),
      createdAt: z.string(),
      status: z.enum(["reserved", "settled", "unresolved", "released"]),
      currency: z.literal("USD"),
      reservedMicros: integer,
      amountMicros: integer.nullable(),
      quotes: z.array(quote).max(64),
      settledQuote: quote.nullable(),
      reason: z.string().nullable(),
      usage: z
        .object({
          prompt_tokens: integer,
          completion_tokens: integer,
          total_tokens: integer,
          cache_creation_input_tokens: integer.nullable(),
          cache_read_input_tokens: integer.nullable(),
        })
        .nullable(),
    }),
  )
  .max(100);
export type CashReceipt = z.infer<typeof cashLedgerSchema>[number];

export function cashBillingApi(client: GatewayApiClient) {
  const root = "/v1/internal/gateway";
  const pricingPath = (id: string) =>
    `${root}/provider-accounts/${encodeURIComponent(id)}/model-pricing`;
  return {
    pricing: (token: string, providerId: string, signal: AbortSignal) =>
      client.request(pricingPath(providerId), accountBillingSchema, {
        managementToken: token,
        signal,
      }),
    // Patch only the selected real account; do not send a stale pricing map back.
    saveMultiplier: (
      token: string,
      providerId: string,
      accountId: string,
      value: string | null,
      signal: AbortSignal,
    ) =>
      client.request(
        pricingPath(providerId),
        z.object({ providerAccount: z.object({ id: z.string() }) }),
        {
          method: "POST",
          managementToken: token,
          signal,
          body: { accountBillingMultipliers: { [accountId]: value } },
        },
      ),
    ledger: (token: string, keyId: string, signal: AbortSignal) =>
      client.request(
        `${root}/access/keys/${encodeURIComponent(keyId)}/cash-ledger`,
        cashLedgerSchema,
        {
          managementToken: token,
          signal,
        },
      ),
  };
}
