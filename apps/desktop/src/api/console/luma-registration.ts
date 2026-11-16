import { z } from "zod";
import { validLumaRedirect } from "./luma-auth";
export const lumaRegistrationSchema = z.object({ registration: z.object({
  id: z.string().uuid(), providerId: z.string().min(1), expectedRevision: z.string().min(1),
  redirectUri: z.string().refine(validLumaRedirect), clientId: z.string().min(1).max(512).nullable(),
  status: z.enum(["prepared", "registering", "registered", "unconfirmed", "cancelled", "expired"]),
  offlineAccess: z.boolean().default(false),
}).refine((value) => (value.status === "registered") === (value.clientId !== null)) });
export type LumaRegistrationView = z.infer<typeof lumaRegistrationSchema>["registration"];
