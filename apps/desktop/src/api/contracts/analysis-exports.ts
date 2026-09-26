// Persisted analysis-export inventory and comparison contracts.

import type { ConsoleJsonObject, ConsoleSummaryBucket } from "./operations-core";

export type ConsoleAnalysisExportFile = {
  kind: string;
  objectKey: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
  lineCount: number | null;
};

export type ConsoleAnalysisExportFilter = {
  projectId: string | null;
  routePolicyId: string | null;
  providerAccountId: string | null;
  sessionId: string | null;
  apiKeyId: string | null;
  responseId: string | null;
  protocolFamily: string | null;
  status: string | null;
  endpointKind: string | null;
  stream: boolean | null;
  errorCode: string | null;
  fallbackEligible: boolean | null;
  createdFrom: string | null;
  createdTo: string | null;
  artifactAvailable: boolean | null;
  limit: number;
  textMode: string;
  maxTextChars: number;
};

export type ConsolePersistedAnalysisExport = {
  exportId: string;
  label: string | null;
  tags: string[];
  status: string;
  createdAt: string;
  updatedAt: string;
  objectPrefix: string;
  filters: ConsoleAnalysisExportFilter;
  sampleCount: number;
  requestArtifactCount: number;
  responseArtifactCount: number;
  retentionExpiresAt: string | null;
  cleanedUpAt: string | null;
  lastCleanupError: string | null;
  files: ConsoleAnalysisExportFile[];
  manifest: ConsoleJsonObject;
};

export type ConsolePersistedAnalysisExportResponse = {
  exports: ConsolePersistedAnalysisExport[];
};

export type ConsoleAnalysisExportInventorySummary = {
  totalExports: number;
  activeExports: number;
  deletedExports: number;
  pinnedExports: number;
  expiringWithin24Hours: number;
  expiredActiveExports: number;
  totalSampleCount: number;
  totalRequestArtifactCount: number;
  totalResponseArtifactCount: number;
  byStatus: ConsoleSummaryBucket[];
  byTextMode: ConsoleSummaryBucket[];
  byTag: ConsoleSummaryBucket[];
  byProject: ConsoleSummaryBucket[];
};

export type ConsoleAnalysisExportInventorySummaryResponse = {
  summary: ConsoleAnalysisExportInventorySummary;
};

export type ConsoleAnalysisExportMetricDelta = {
  leftValue: number | null;
  rightValue: number | null;
  deltaValue: number | null;
};

export type ConsoleAnalysisExportBucketDelta = {
  key: string;
  leftCount: number;
  rightCount: number;
  deltaCount: number;
};

export type ConsoleAnalysisExportDiff = {
  leftExport: ConsolePersistedAnalysisExport;
  rightExport: ConsolePersistedAnalysisExport;
  overlapRequestCount: number;
  leftOnlyRequestCount: number;
  rightOnlyRequestCount: number;
  sampleCount: ConsoleAnalysisExportMetricDelta;
  requestArtifactCount: ConsoleAnalysisExportMetricDelta;
  responseArtifactCount: ConsoleAnalysisExportMetricDelta;
  promptTokens: ConsoleAnalysisExportMetricDelta;
  completionTokens: ConsoleAnalysisExportMetricDelta;
  totalTokens: ConsoleAnalysisExportMetricDelta;
  byStatus: ConsoleAnalysisExportBucketDelta[];
  byProtocolFamily: ConsoleAnalysisExportBucketDelta[];
  byEndpointKind: ConsoleAnalysisExportBucketDelta[];
  byResolvedModel: ConsoleAnalysisExportBucketDelta[];
  byProviderAccount: ConsoleAnalysisExportBucketDelta[];
};

export type ConsoleAnalysisExportDiffResponse = {
  diff: ConsoleAnalysisExportDiff;
};
