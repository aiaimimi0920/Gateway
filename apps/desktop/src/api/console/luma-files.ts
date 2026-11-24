import { z } from "zod";

export const LUMA_FILE_LIMIT = 5 * 1024 * 1024 * 1024;
export const LUMA_INLINE_FILE_LIMIT = 8 * 1024 * 1024;
export const lumaFileSchema = z.object({
  id: z.string().uuid(), state: z.enum(["pending", "ready", "failed", "deleted"]),
  filename: z.string().max(1024).nullable(), mime_type: z.string().min(1).max(255),
  size_bytes: z.number().int().nonnegative().max(5 * 1024 * 1024 * 1024),
  purpose: z.enum(["input", "reference"]), created_at: z.string().max(64),
  expires_at: z.string().max(64).nullable().optional(), user_id: z.string().max(256).nullable().optional(),
});
export const lumaFilesSchema = z.object({
  revision: z.string(), providerId: z.string(), credentialId: z.string(),
  result: z.discriminatedUnion("action", [
    z.object({ action: z.literal("list"), data: z.array(lumaFileSchema).max(25), has_more: z.boolean(), next_cursor: z.string().min(1).max(4096).nullable() })
      .refine((p) => !p.has_more || (!!p.next_cursor && p.data.length > 0))
      .refine((p) => new Set(p.data.map((f) => f.id)).size === p.data.length),
    z.object({ action: z.literal("get"), file: lumaFileSchema }),
    z.object({ action: z.literal("upload"), file: lumaFileSchema }),
    z.object({ action: z.literal("delete"), id: z.string().uuid() }),
  ]),
});
export type LumaFile = z.infer<typeof lumaFileSchema>;
export type LumaFileOperation = { action: "list"; cursor?: string } | { action: "get" | "delete"; id: string }
  | { action: "upload"; filename: string; purpose: "input" | "reference"; data: string };
