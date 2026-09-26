// Shared console request and incident mutation filters.

/** Operator note attached to an anomaly incident; every field is optional-patch. */
export type ConsoleAnomalyIncidentFollowUpInput = {
  ownerUserId?: string | null;
  followUpStatus?: string | null;
  note?: string | null;
  resolutionNote?: string | null;
};

/**
 * Shared query filters for `/requests`, `/analysis/samples` and the export
 * endpoints. Only the fields the console actually surfaces are modelled.
 */
export type ConsoleRequestAuditFilters = {
  projectId?: string;
  routePolicyId?: string;
  providerAccountId?: string;
  sessionId?: string;
  accessKeyId?: string;
  responseId?: string;
  protocolFamily?: string;
  status?: string;
  endpointKind?: string;
  errorCode?: string;
  stream?: boolean;
  fallbackEligible?: boolean;
  artifactAvailable?: boolean;
  createdFrom?: string;
  createdTo?: string;
  limit?: number;
};
