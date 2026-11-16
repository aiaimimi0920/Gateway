import { z } from "zod";
import { freebuffBusinessOrigin, freebuffClient } from "./freebuffCatalog";
const binding = z.object({
  revision: z.string().max(1024), providerId: z.string().max(1024), credentialId: z.string().max(1024),
  businessOrigin: freebuffBusinessOrigin, client: freebuffClient,
});
// IDs identify local records/attempts, never the upstream device key or a reusable secret.
export const freebuffDeviceResponse = z.discriminatedUnion("registration", [
  binding.extend({ registration: z.literal("absent") }).strict(),
  binding.extend({ registration: z.literal("stored"), recordId: z.uuid() }).strict(),
  binding.extend({ registration: z.literal("unconfirmed"), unconfirmedAttemptId: z.uuid() }).strict(),
]);
export type FreebuffDeviceState = z.infer<typeof freebuffDeviceResponse>;
