import { z } from "zod";
import { freebuffBusinessOrigin, freebuffClient } from "./freebuffCatalog";
import { freebuffRun } from "./freebuffRun";
const amount = z.number().finite().nonnegative();
const id = z.uuid().refine(value => value === value.toLowerCase() && value[14] === "4");
const model = z.string().min(1).max(1024).refine(value => !/[\u0000-\u001f\u007f]/.test(value));
const receipt = z.object({ refund: amount.nullable(), refundSources: z.object({ daily: amount, wallet: amount }).strict().nullable(),
  refundPending: z.boolean(), inGrace: z.boolean() }).strict();
const final = (value: z.infer<typeof receipt>) => value.refund !== null && !value.refundPending && !value.inGrace;
const identity = { revision: z.string().min(1), providerId: z.string().min(1), credentialId: z.string().min(1),
  businessOrigin: freebuffBusinessOrigin, client: freebuffClient };
export const freebuffClaimJournal = z.object({ ...identity, cursor: id.nullable(), claim: z.object({ attemptId: id, modelKey: model, run: freebuffRun.nullable().optional() }).strict().nullable(),
  history: z.array(z.object({ attemptId: id, modelKey: model, receipt: receipt.refine(final), run: freebuffRun.nullable().optional() }).strict()).max(20), nextCursor: id.nullable() }).strict()
  .refine(value => new Set(value.history.map(row => row.attemptId)).size === value.history.length
    && (value.nextCursor === null || (value.history.length === 20 && value.nextCursor === value.history.at(-1)?.attemptId)));
export const freebuffClaimEnd = z.object({ ...identity, attemptId: id, modelKey: model, receipt, localReleased: z.boolean(), fromHistory: z.boolean() }).strict()
  .refine(value => value.localReleased === final(value.receipt) && (!value.fromHistory || value.localReleased));
export type FreebuffClaimJournal = z.infer<typeof freebuffClaimJournal>;
export type FreebuffClaimEnd = z.infer<typeof freebuffClaimEnd>;
