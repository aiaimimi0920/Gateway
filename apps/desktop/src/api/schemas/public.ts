import { z } from "zod";

import type { PublicHealth, PublicModelList, PublicReadiness } from "../contracts";

export const publicHealthSchema: z.ZodType<PublicHealth> = z
  .object({ status: z.string().min(1) })
  .catchall(z.unknown());

export const publicReadinessSchema: z.ZodType<PublicReadiness> = z
  .object({
    ready: z.boolean().optional(),
    status: z.string().min(1),
  })
  .catchall(z.unknown());

const publicModelSchema = z
  .object({
    id: z.string().min(1),
    object: z.string().optional(),
    created: z.number().optional(),
    owned_by: z.string().optional(),
    ownedBy: z.string().optional(),
  })
  .catchall(z.unknown());

export const publicModelListSchema: z.ZodType<PublicModelList> = z
  .object({
    object: z.string().optional(),
    data: z.array(publicModelSchema),
  })
  .catchall(z.unknown());

export const unknownObjectSchema = z.record(z.string(), z.unknown());
