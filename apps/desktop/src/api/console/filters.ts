type QueryValue = string | number | boolean | undefined | null;

/** Filters shared by the `/requests`-family endpoints. */
export type ConsoleRequestFilters = {
  projectId?: string;
  routePolicyId?: string;
  providerAccountId?: string;
  sessionId?: string;
  apiKeyId?: string;
  userCredentialId?: string;
  accessKeyId?: string;
  responseId?: string;
  protocolFamily?: string;
  status?: string;
  endpointKind?: string;
  stream?: boolean;
  errorCode?: string;
  fallbackEligible?: boolean;
  artifactAvailable?: boolean;
  createdFrom?: string;
  createdTo?: string;
  limit?: number;
};

/** Mirrors `AnomalyIncidentQuery`. */
export type ConsoleAnomalyFilters = {
  incidentId?: string;
  policyId?: string;
  projectId?: string;
  routePolicyId?: string;
  ownerUserId?: string;
  tag?: string;
  status?: string;
  followUpStatus?: string;
  escalationStatus?: string;
  code?: string;
  severity?: string;
  dueOnly?: boolean;
  limit?: number;
};

/** Mirrors `AnomalyPolicyQuery`. */
export type ConsoleAnomalyPolicyFilters = {
  policyId?: string;
  projectId?: string;
  routePolicyId?: string;
  status?: string;
  tag?: string;
  autoSyncEnabled?: boolean;
  autoEscalateEnabled?: boolean;
  autoRemediationEnabled?: boolean;
  alertingEnabled?: boolean;
  dueOnly?: boolean;
  limit?: number;
};

/** Mirrors `RemediationRunQuery` and `RemediationQueueQuery`. */
export type ConsoleRemediationFilters = {
  incidentId?: string;
  policyId?: string;
  routePolicyId?: string;
  actionKey?: string;
  status?: string;
  executionMode?: string;
  dryRun?: boolean;
  createdFrom?: string;
  createdTo?: string;
  dueOnly?: boolean;
  limit?: number;
};

/** Mirrors `PersistedAnalysisExportQuery`. */
export type ConsoleAnalysisExportFilters = {
  exportId?: string;
  label?: string;
  tag?: string;
  projectId?: string;
  status?: string;
  textMode?: string;
  createdFrom?: string;
  createdTo?: string;
  limit?: number;
};

/** Drops empty values so the gateway sees an absent filter, not an empty one. */
export function buildQuery(
  params: Record<string, QueryValue> = {},
  defaults: Record<string, QueryValue> = {},
): string {
  const merged: Record<string, QueryValue> = { ...defaults };
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined) {
      merged[key] = value;
    }
  }
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(merged)) {
    if (value === undefined || value === null || value === "") {
      continue;
    }
    query.set(key, String(value));
  }
  const encoded = query.toString();
  return encoded.length > 0 ? `?${encoded}` : "";
}
