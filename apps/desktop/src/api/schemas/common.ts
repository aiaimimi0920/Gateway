import { z } from "zod";

// Shared normalization leaves used by multiple wire-schema domains.
export const consoleNullableStringSchema = z
  .string()
  .nullish()
  .transform((value) => value ?? null);

export const consoleNullableTimestampSchema = z
  .string()
  .nullish()
  .transform((value) => value ?? null);

export const consoleJsonObjectSchema = z
  .record(z.string(), z.unknown())
  .nullish()
  .transform((value) => value ?? {});

export const consoleNullableJsonObjectSchema = z
  .record(z.string(), z.unknown())
  .nullish()
  .transform((value) => value ?? null);

export const consoleNullableNumberSchema = z
  .number()
  .nullish()
  .transform((value) => value ?? null);

export const consoleNullableBooleanSchema = z
  .boolean()
  .nullish()
  .transform((value) => value ?? null);

export const consoleSummaryBucketsSchema = z
  .array(z.object({ key: z.string(), count: z.number().int().nonnegative() }))
  .nullish()
  .transform((value) => value ?? []);

export const consoleValueBucketsSchema = z
  .array(z.object({ value: z.string(), count: z.number().int().nonnegative() }))
  .nullish()
  .transform((value) => value ?? []);

export const consoleStringArraySchema = z
  .array(z.string())
  .nullish()
  .transform((value) => value ?? []);
