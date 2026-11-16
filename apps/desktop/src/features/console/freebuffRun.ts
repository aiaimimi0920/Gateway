import { z } from "zod";
const count = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER);
const amount = z.number().finite().nonnegative();
const totals = z.object({ totalSteps: count, directCredits: count, totalCredits: count }).strict()
  .refine(value => value.directCredits === value.totalCredits);
const cost = z.object({ cost: amount.nullable(), upstreamEstimate: amount.nullable(), sdkCredits: count.nullable() }).strict()
  .refine(value => value.cost !== null || value.upstreamEstimate !== null);
/** Facts only; remote run/client identities and credentials are deliberately absent. */
export const freebuffRun = z.object({
  phase: z.enum(["start_pending", "started", "model_pending", "model_observed", "step_stopped", "finish_pending", "finish_accepted"]),
  runIdRecorded: z.boolean(), totals: totals.nullable(), priorTotals: totals.nullable().optional(), lastStep: cost.nullable(),
  lastError: z.string().regex(/^[\x21-\x7e]{1,128}$/).nullable(),
  finishStatus: z.enum(["completed", "cancelled", "failed"]).nullable(),
  finishHttpStatus: z.number().int().min(200).max(299).nullable(),
}).strict().refine(value => {
  const finishing = value.phase === "finish_pending" || value.phase === "finish_accepted";
  return value.runIdRecorded === (value.phase !== "start_pending")
    && finishing === (value.finishStatus !== null) && (!finishing || value.totals !== null)
    && (value.phase === "finish_accepted") === (value.finishHttpStatus !== null)
    && (value.phase !== "model_pending" || value.totals === null)
    && (!["start_pending", "started"].includes(value.phase) || (value.totals?.totalSteps === 0 && value.totals.totalCredits === 0));
});
export type FreebuffRun = z.infer<typeof freebuffRun>;
