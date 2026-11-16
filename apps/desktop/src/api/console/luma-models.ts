// Model metadata is untrusted display data, not execution instructions or a settled bill.
import { z } from "zod";
const object = z.record(z.string(), z.unknown());
export const lumaModelSchema = z.object({
  id: z.string().min(1).max(128), aliases: z.array(z.string()),
  modality: z.enum(["image", "video", "audio", "edit", "utility"]), actions: z.array(object),
  capabilities: object, pricing: object.nullable(), strengths: z.string().nullable(),
});
const binding = { revision: z.string(), providerId: z.string(), credentialId: z.string() };
export const lumaModelsSchema = z.object({ ...binding, result: z.object({ source: z.literal("registry"),
  models: z.array(lumaModelSchema).max(256).refine((rows) => new Set(rows.map((row) => row.id)).size === rows.length) }) });
export const lumaModelDetailsSchema = z.object({ ...binding, result: z.object({
  model: z.string(), type: z.enum(["image", "video"]), parameters: object, media_roles: z.array(z.string()), guidance: z.string().nullable(),
}) });
export type LumaModel = z.infer<typeof lumaModelSchema>;
export type LumaModelDetails = z.infer<typeof lumaModelDetailsSchema>["result"];
