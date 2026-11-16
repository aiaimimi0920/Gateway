// Display snapshots are not admission, ownership, a locked quote or a debit receipt.
import { z } from "zod";
import { freebuffBusinessOrigin, freebuffClient } from "./freebuffCatalog";
import { validModelId } from "./providerModelId";
const amount = z.number().finite().nonnegative();
const modelKey = z.string().refine(validModelId);
const instant = z.string().datetime({ offset: true });
const session = z.discriminatedUnion("status", [
  z.object({ status: z.literal("none") }).strict(),
  z.object({ status: z.literal("active"), modelKey, admittedAt: instant, expiresAt: instant, remainingMs: amount }).strict(),
  z.object({ status: z.literal("ended"), inGrace: z.boolean() }).strict(),
]);
const meter = z.discriminatedUnion("state", [
  z.object({ state: z.literal("absent") }).strict(),
  z.object({ state: z.literal("unavailable") }).strict(),
  z.object({ state: z.literal("available"), balance: amount, dailyRemaining: amount, walletBalance: amount,
    sessionPrice: amount.nullable(), claimableGrant: amount.optional(), soloDiscount: amount.optional(),
    quotaExempt: z.boolean().optional(), spendWithoutAsking: z.boolean().optional() }).strict(),
]);
export const freebuffInspectionResponse = z.object({ revision: z.string().min(1), providerId: z.string().min(1),
  credentialId: z.string().min(1), businessOrigin: freebuffBusinessOrigin, client: freebuffClient,
  modelKey, observedAt: instant, session, meter }).strict();
export type FreebuffInspection = z.infer<typeof freebuffInspectionResponse>;
