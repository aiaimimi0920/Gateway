import { z } from "zod";

import type {
  ConsoleAlertQueueResponse,
  ConsoleAnomalyIncidentHistoryResponse,
  ConsoleAnomalyIncidentMutationResponse,
  ConsoleAnomalyIncidentResponse,
  ConsoleAnomalyIncidentSummaryResponse,
  ConsoleAnomalyPolicyMutationResponse,
  ConsoleAnomalyPolicyResponse,
  ConsoleRemediationEffectivenessResponse,
  ConsoleRemediationQueueResponse,
  ConsoleRemediationRunResponse,
} from "../contracts";
import {
  consoleJsonObjectSchema,
  consoleNullableJsonObjectSchema,
  consoleNullableNumberSchema,
  consoleNullableStringSchema,
  consoleNullableTimestampSchema,
  consoleStringArraySchema,
  consoleSummaryBucketsSchema,
} from "./common";

const consoleAnomalyIncidentSchema = z.object({
  id: z.string().min(1),
  policyId: consoleNullableStringSchema,
  fingerprint: z.string(),
  projectId: consoleNullableStringSchema,
  routePolicyId: consoleNullableStringSchema,
  tag: consoleNullableStringSchema,
  textMode: consoleNullableStringSchema,
  code: z.string(),
  severity: z.string(),
  status: z.string(),
  ownerUserId: consoleNullableStringSchema,
  followUpStatus: z.string(),
  syncHitCount: z.number().int(),
  escalationStatus: z.string(),
  escalatedAt: consoleNullableTimestampSchema,
  escalationReason: consoleNullableStringSchema,
  latestNote: consoleNullableStringSchema,
  resolutionNote: consoleNullableStringSchema,
  lastActionAt: consoleNullableTimestampSchema,
  lastAlertAttemptAt: consoleNullableTimestampSchema,
  lastAlertedAt: consoleNullableTimestampSchema,
  lastAlertSeverity: consoleNullableStringSchema,
  alertDeliveryCount: z.number().int(),
  summary: z.string(),
  latestExportId: consoleNullableStringSchema,
  previousExportId: consoleNullableStringSchema,
  latestValue: consoleNullableNumberSchema,
  previousValue: consoleNullableNumberSchema,
  deltaValue: consoleNullableNumberSchema,
  deltaRatio: consoleNullableNumberSchema,
  thresholdValue: consoleNullableNumberSchema,
  firstSeenAt: z.string().min(1),
  lastSeenAt: z.string().min(1),
  acknowledgedAt: consoleNullableTimestampSchema,
  resolvedAt: consoleNullableTimestampSchema,
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
});

export const consoleAnomalyIncidentResponseSchema: z.ZodType<ConsoleAnomalyIncidentResponse> =
  z.object({ incidents: z.array(consoleAnomalyIncidentSchema) });

export const consoleAnomalyIncidentMutationResponseSchema: z.ZodType<ConsoleAnomalyIncidentMutationResponse> =
  z.object({ incident: consoleAnomalyIncidentSchema });

export const consoleAnomalyIncidentSummaryResponseSchema: z.ZodType<ConsoleAnomalyIncidentSummaryResponse> =
  z.object({
    summary: z.object({
      totalIncidents: z.number().int().nonnegative(),
      openIncidents: z.number().int().nonnegative(),
      acknowledgedIncidents: z.number().int().nonnegative(),
      resolvedIncidents: z.number().int().nonnegative(),
      escalatedIncidents: z.number().int().nonnegative(),
      byStatus: consoleSummaryBucketsSchema,
      bySeverity: consoleSummaryBucketsSchema,
      byCode: consoleSummaryBucketsSchema,
      byFollowUpStatus: consoleSummaryBucketsSchema,
      byEscalationStatus: consoleSummaryBucketsSchema,
    }),
  });

export const consoleAnomalyIncidentHistoryResponseSchema: z.ZodType<ConsoleAnomalyIncidentHistoryResponse> =
  z.object({
    history: z.array(
      z.object({
        id: z.string().min(1),
        incidentId: z.string().min(1),
        eventType: z.string(),
        actorUserId: consoleNullableStringSchema,
        note: consoleNullableStringSchema,
        metadata: consoleNullableJsonObjectSchema,
        createdAt: z.string().min(1),
      }),
    ),
  });

export const consoleAlertQueueResponseSchema: z.ZodType<ConsoleAlertQueueResponse> = z.object({
  queue: z.object({
    generatedAt: z.string().min(1),
    limit: z.number().int().nonnegative(),
    dueOnly: z.boolean(),
    incidentCount: z.number().int().nonnegative(),
    dueCount: z.number().int().nonnegative(),
    items: z.array(
      z.object({
        incident: consoleAnomalyIncidentSchema,
        policy: consoleNullableJsonObjectSchema,
        routePolicy: consoleNullableJsonObjectSchema,
        alertIntervalMinutes: z.number().int(),
        alertDue: z.boolean(),
        nextAlertDueAt: consoleNullableTimestampSchema,
        notifyOperators: z.boolean(),
        notifyOwner: z.boolean(),
        alertLevel: z.number().int(),
        webhookSeverity: z.string(),
        remediationActionKeys: consoleStringArraySchema,
      }),
    ),
  }),
});

const consoleAnomalyPolicySchema = z.object({
  id: z.string().min(1),
  name: z.string(),
  status: z.string(),
  projectId: consoleNullableStringSchema,
  routePolicyId: consoleNullableStringSchema,
  tag: consoleNullableStringSchema,
  textMode: consoleNullableStringSchema,
  profileKey: z.string(),
  thresholds: consoleJsonObjectSchema,
  autoSyncEnabled: z.boolean(),
  autoSyncIntervalMinutes: consoleNullableNumberSchema,
  lastSyncedAt: consoleNullableTimestampSchema,
  lastSyncStatus: consoleNullableStringSchema,
  lastSyncError: consoleNullableStringSchema,
  nextSyncDueAt: consoleNullableTimestampSchema,
  syncDue: z.boolean(),
  autoEscalateEnabled: z.boolean(),
  escalateSeverityThreshold: consoleNullableStringSchema,
  escalateAfterSyncCount: consoleNullableNumberSchema,
  autoEscalateOwnerUserId: consoleNullableStringSchema,
  autoEscalateFollowUpStatus: consoleNullableStringSchema,
  autoRemediationEnabled: z.boolean(),
  autoRemediationIntervalMinutes: consoleNullableNumberSchema,
  autoRemediationDryRunFirst: z.boolean(),
  autoRemediationActionKeys: consoleStringArraySchema,
  autoRemediationMaxApplyRunsPerIncident: consoleNullableNumberSchema,
  autoRemediationRequireAlertBeforeApply: z.boolean(),
  autoRemediationFreezeOnProviderHealthDegrade: z.boolean(),
  alertingEnabled: z.boolean(),
  alertIntervalMinutes: consoleNullableNumberSchema,
  notifyOperatorsOnEscalation: z.boolean(),
  notifyOwnerOnEscalation: z.boolean(),
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
});

export const consoleAnomalyPolicyResponseSchema: z.ZodType<ConsoleAnomalyPolicyResponse> = z.object(
  { policies: z.array(consoleAnomalyPolicySchema) },
);

export const consoleAnomalyPolicyMutationResponseSchema: z.ZodType<ConsoleAnomalyPolicyMutationResponse> =
  z.object({ policy: consoleAnomalyPolicySchema });

const consoleRemediationRunSchema = z.object({
  id: z.string().min(1),
  incidentId: z.string().min(1),
  policyId: consoleNullableStringSchema,
  routePolicyId: consoleNullableStringSchema,
  actionKey: z.string(),
  title: z.string(),
  executionMode: z.string(),
  status: z.string(),
  dryRun: z.boolean(),
  actorUserId: z.string(),
  note: consoleNullableStringSchema,
  input: consoleNullableJsonObjectSchema,
  result: consoleNullableJsonObjectSchema,
  errorSummary: consoleNullableStringSchema,
  createdAt: z.string().min(1),
  completedAt: consoleNullableTimestampSchema,
});

export const consoleRemediationRunResponseSchema: z.ZodType<ConsoleRemediationRunResponse> =
  z.object({ runs: z.array(consoleRemediationRunSchema) });

export const consoleRemediationQueueResponseSchema: z.ZodType<ConsoleRemediationQueueResponse> =
  z.object({
    queue: z.object({
      generatedAt: z.string().min(1),
      limit: z.number().int().nonnegative(),
      dueOnly: z.boolean(),
      itemCount: z.number().int().nonnegative(),
      dueCount: z.number().int().nonnegative(),
      items: z.array(
        z.object({
          incident: consoleAnomalyIncidentSchema,
          policy: consoleNullableJsonObjectSchema,
          routePolicy: consoleNullableJsonObjectSchema,
          action: z.object({
            actionKey: z.string(),
            title: z.string(),
            description: z.string(),
            category: z.string(),
            priority: z.string(),
            routePolicyId: consoleNullableStringSchema,
            executable: z.boolean(),
            executionMode: z.string(),
            defaultExecutionInput: consoleNullableJsonObjectSchema,
            recommendedChanges: consoleNullableJsonObjectSchema,
          }),
          remediationDue: z.boolean(),
          nextExecutionStatus: consoleNullableStringSchema,
          nextRunDueAt: consoleNullableTimestampSchema,
          blockedReason: consoleNullableStringSchema,
          latestRun: consoleRemediationRunSchema.nullish().transform((value) => value ?? null),
        }),
      ),
    }),
  });

const consoleRemediationMetricSchema = z.object({
  improvedRuns: z.number().int().nonnegative(),
  regressedRuns: z.number().int().nonnegative(),
  neutralRuns: z.number().int().nonnegative(),
  unavailableRuns: z.number().int().nonnegative(),
});

export const consoleRemediationEffectivenessResponseSchema: z.ZodType<ConsoleRemediationEffectivenessResponse> =
  z.object({
    effectiveness: z.object({
      generatedAt: z.string().min(1),
      windowMinutes: z.number().int(),
      totalRuns: z.number().int().nonnegative(),
      impactedRuns: z.number().int().nonnegative(),
      unavailableRuns: z.number().int().nonnegative(),
      byStatus: consoleSummaryBucketsSchema,
      byExecutionMode: consoleSummaryBucketsSchema,
      byActionKey: consoleSummaryBucketsSchema,
      completionRate: consoleRemediationMetricSchema,
      failureRate: consoleRemediationMetricSchema,
      requestArtifactCoverage: consoleRemediationMetricSchema,
      responseArtifactCoverage: consoleRemediationMetricSchema,
      firstTokenLatencyMsAvg: consoleRemediationMetricSchema,
      totalTokensPerSample: consoleRemediationMetricSchema,
      actions: z
        .array(
          z.object({
            actionKey: z.string(),
            runCount: z.number().int().nonnegative(),
            impactedRunCount: z.number().int().nonnegative(),
            unavailableRunCount: z.number().int().nonnegative(),
            completionRate: consoleRemediationMetricSchema,
            failureRate: consoleRemediationMetricSchema,
            requestArtifactCoverage: consoleRemediationMetricSchema,
            responseArtifactCoverage: consoleRemediationMetricSchema,
            firstTokenLatencyMsAvg: consoleRemediationMetricSchema,
            totalTokensPerSample: consoleRemediationMetricSchema,
          }),
        )
        .nullish()
        .transform((value) => value ?? []),
    }),
  });
