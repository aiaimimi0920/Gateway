import { z } from "zod";
import type {
  BootstrapStatus,
  ConsoleEvent,
  ManagementSession,
  OperationSuccess,
  PublicHealth,
  PublicModelList,
  PublicReadiness,
  SecretGrant,
} from "./contracts";

export const bootstrapStatusSchema: z.ZodType<BootstrapStatus> = z
  .object({
    needsBootstrap: z.boolean(),
    managementConfigured: z.boolean().optional(),
    environmentOverride: z.boolean().optional(),
  })
  .transform((value) => ({
    needsBootstrap: value.needsBootstrap,
    managementConfigured: value.managementConfigured ?? !value.needsBootstrap,
    environmentOverride: value.environmentOverride ?? false,
  }));

export const managementSessionSchema: z.ZodType<ManagementSession> = z.object({
  role: z.string().min(1),
  capabilities: z.array(z.string()),
  activeRevision: z.string().min(1).nullable(),
  secretAccessGranted: z.boolean(),
});

export const secretGrantSchema: z.ZodType<SecretGrant> = z.object({
  grant: z.string().min(1),
  expiresAt: z.string().min(1),
});

export const operationSuccessSchema: z.ZodType<OperationSuccess> = z
  .union([
    z.object({ success: z.literal(true), message: z.string().optional() }),
    z.object({ ok: z.literal(true), message: z.string().optional() }),
  ])
  .transform((value) => ({ success: true as const, message: value.message }));

export const consoleEventSchema: z.ZodType<ConsoleEvent> = z.object({
  id: z.union([z.string(), z.number()]).transform(String),
  kind: z.string().min(1),
  timestamp: z.string().min(1),
  message: z.string().optional(),
  data: z.record(z.string(), z.unknown()).optional().default({}),
});

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
