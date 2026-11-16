import { z } from "zod";
export const freebuffBusinessOrigin = z.enum(["https://codebuff.com", "https://www.codebuff.com"]);
export const freebuffClient = z.enum(["cli", "desktop"]);
const text = (max: number) => z.string().max(max);
// Display-only DTO: short-lived handles/fetchId and secret material are never accepted here.
export const freebuffCatalogResponse = z.object({
  revision: text(1024), providerId: text(1024), credentialId: text(1024),
  businessOrigin: freebuffBusinessOrigin, client: freebuffClient,
  models: z.array(z.object({
    key: text(1024), displayName: text(4096), tagline: text(8192), warning: text(8192).nullable(),
    access: z.enum(["open", "locked"]), dataUse: z.enum(["service", "training"]),
    premium: z.boolean(), multimodal: z.boolean(),
    badges: z.array(z.object({ kind: text(1024), label: text(4096) }).strict()).max(64),
  }).strict()).min(1).max(512),
}).strict();
export type FreebuffCatalog = z.infer<typeof freebuffCatalogResponse>;
