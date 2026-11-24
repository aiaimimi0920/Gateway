import { z } from "zod";

export const lumaTaskIdSchema = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i).transform(value => value.toLowerCase());
// Whitelist the supplier facts; arbitrary output URLs, prompts and diagnostics are not UI data.
export const lumaTaskSchema = z.object({
  id: lumaTaskIdSchema,
  model: z.string().min(1).max(128).refine(value => !/[\x00-\x1f\x7f]/.test(value)),
  type: z.enum(["image", "image_edit", "video", "video_edit", "video_reframe", "layering"]),
  state: z.enum(["queued", "processing", "completed", "failed"]),
  usage: z.object({ credits_charged: z.number().finite().nonnegative().nullable() }),
});
export const lumaTaskResponseSchema = z.object({
  revision: z.string(), providerId: z.string(), credentialId: z.string(), result: lumaTaskSchema,
});
export const lumaTaskAuditSchema = lumaTaskSchema.extend({ mediaVerified: z.boolean().optional() });
export type LumaTask = z.infer<typeof lumaTaskSchema>;
