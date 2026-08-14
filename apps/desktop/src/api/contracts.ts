export type BootstrapStatus = {
  needsBootstrap: boolean;
  managementConfigured: boolean;
  environmentOverride: boolean;
};

export type ManagementSession = {
  role: string;
  capabilities: string[];
  activeRevision: string | null;
  secretAccessGranted: boolean;
};

export type SecretGrant = {
  grant: string;
  expiresAt: string;
};

export type OperationSuccess = {
  success: true;
  message?: string;
};

export type ConsoleRevisionMetadata = {
  id: string;
  sequence: number;
  parent?: string | null;
  actor?: string;
  timestamp?: string;
  documentDigest?: string;
  yamlDigest?: string;
  message?: string | null;
};

export type ConsoleSecretDescriptor = {
  path: string;
  kind?: string;
  configured?: boolean;
  preview?: string | null;
  fingerprint?: string | null;
  [key: string]: unknown;
};

export type ConsoleRouteDocument = {
  providers: unknown[];
  model_routes: unknown[];
  aliases: Record<string, string>;
  [key: string]: unknown;
};

export type ConsoleRouteConfigView = {
  revision: ConsoleRevisionMetadata;
  source: string;
  diagnostics: {
    diagnostics: Array<{
      code: string;
      severity: string;
      path: string;
      message: string;
    }>;
  } | null;
  requiresRepair: boolean;
  document: ConsoleRouteDocument;
  secrets: ConsoleSecretDescriptor[];
  mutationSupported: boolean;
};

export type ConsoleRouteConfigResponse = {
  routeConfig: ConsoleRouteConfigView;
};

export type ConsoleRouteConfigCommitResponse = ConsoleRouteConfigResponse & {
  committed: boolean;
};

export type ConsoleRouteConfigValidationRequest = {
  document: ConsoleRouteDocument;
  secretPatches: ConsoleSecretPatch[];
};

export type ConsoleRouteConfigCommitRequest = ConsoleRouteConfigValidationRequest & {
  expectedRevision: string;
  message?: string;
};

export type ConsoleRouteConfigValidationResponse = {
  validation: {
    document: ConsoleRouteDocument;
    secrets: ConsoleSecretDescriptor[];
    diagnostics: {
      diagnostics: Array<{
        code: string;
        severity: string;
        path: string;
        message: string;
      }>;
    };
    requiresRepair: boolean;
  };
};

export type ConsoleRouteRevisionListEntry = {
  revision: ConsoleRevisionMetadata;
  active: boolean;
  hasArchive: boolean;
  source: string;
};

export type ConsoleRouteRevisionListResponse = {
  revisions: ConsoleRouteRevisionListEntry[];
};

export type ConsoleRouteRevisionDetailResponse = {
  routeConfig: ConsoleRouteConfigView;
  active: boolean;
  hasArchive: boolean;
};

export type ConsoleAccountGroupSummaryGroup = {
  id: string;
  name: string;
  description?: string | null;
  billingMultiplier: number;
  configuredBillingMultiplier?: number | null;
  enabled: boolean;
  notes?: string | null;
  memberCount: number;
  providerCredentialIds: string[];
  providers: string[];
};

export type ConsoleAccountGroupSummaryAccount = {
  id: string;
  displayName: string;
  providerId: string;
  providerLabel: string;
  /** Stable vendor identifier; omitted by legacy route documents. */
  vendorKey?: string | null;
  /** Human-readable vendor name; omitted by legacy route documents. */
  vendorName?: string | null;
  providerPreset?: string | null;
  credentialId?: string | null;
  baseUrl?: string | null;
  mode: string;
  /** Effective routing status; legacy summaries may omit this field. */
  enabled: boolean;
  supportedModels: string[];
  groupIds: string[];
};

export type ConsoleAccountGroupSummaryProvider = {
  id: string;
  label: string;
  /** Stable vendor identifier; omitted by legacy route documents. */
  vendorKey?: string | null;
  /** Human-readable vendor name; omitted by legacy route documents. */
  vendorName?: string | null;
  preset?: string | null;
  baseUrl?: string | null;
  accountIds: string[];
  supportedModels: string[];
};

export type ConsoleAccountGroupSummary = {
  routeConfigRevision: string;
  source: string;
  accountGroups: ConsoleAccountGroupSummaryGroup[];
  accounts: ConsoleAccountGroupSummaryAccount[];
  providers: ConsoleAccountGroupSummaryProvider[];
};

export type ConsoleAccountGroupSummaryResponse = {
  summary: ConsoleAccountGroupSummary;
};

export type ConsoleCredentialPoolAutomationDriver = {
  id: string;
  mode: "script" | "http";
  providerIds: string[];
};

export type ConsoleCredentialPoolAutomationProvider = {
  providerId: string;
  providerLabel: string;
  targetSize: number;
  credentialCount: number;
  activeCredentialCount: number;
  autoRefillEnabled: boolean;
  autoPruneEnabled: boolean;
  driverId: string | null;
  driverMode: "script" | "http" | null;
  driverConfigured: boolean;
  state: "disabled" | "not_configured" | "idle" | "running" | "succeeded" | "failed";
  lastRunAt: string | null;
  nextRunAt: string | null;
  lastAction: string | null;
  createdCount: number;
  prunedCount: number;
  message: string | null;
  revisionId: string | null;
};

export type ConsoleCredentialPoolAutomationResponse = {
  automation: {
    enabled: boolean;
    intervalSeconds: number;
    drivers: ConsoleCredentialPoolAutomationDriver[];
    providers: ConsoleCredentialPoolAutomationProvider[];
    revisionId: string;
  };
};

export type ConsoleCredentialPoolAutomationRunResponse = {
  provider: ConsoleCredentialPoolAutomationProvider;
};

export type ConsoleCredentialRefillTrigger = "notification" | "inquiry" | "user_requested";
export type ConsoleCredentialRefillTaskState = "pending" | "claimed" | "succeeded" | "failed";
export type ConsoleCredentialRefillDeliveryMode =
  | "folder_sync"
  | "gateway_pull"
  | "direct_callback";

export type ConsoleCredentialRefillDemand = {
  providerId: string;
  providerLabel: string;
  targetSize: number;
  credentialCount: number;
  activeCredentialCount: number;
  deficit: number;
  needsRefill: boolean;
  autoRefillEnabled: boolean;
  directDriverConfigured: boolean;
  notificationEnabled: boolean;
  inquiryEnabled: boolean;
  userRequestEnabled: boolean;
  outstandingTaskId: string | null;
  outstandingTaskState: ConsoleCredentialRefillTaskState | null;
  revisionId: string;
};

export type ConsoleCredentialRefillTask = {
  id: string;
  providerId: string;
  providerLabel: string;
  trigger: ConsoleCredentialRefillTrigger;
  state: ConsoleCredentialRefillTaskState;
  requestedCount: number;
  targetSize: number;
  activeCredentialCount: number;
  routeRevision: string;
  createdAt: string;
  updatedAt: string;
  workerId: string | null;
  leaseUntil: string | null;
  attempt: number;
  deliveryMode: ConsoleCredentialRefillDeliveryMode | null;
  createdCount: number;
  message: string | null;
  revisionId: string | null;
};

export type ConsoleCredentialRefillResponse = {
  refill: {
    enabled: boolean;
    streamKey: string;
    notificationIntervalSeconds: number;
    defaultLeaseSeconds: number;
    maxLeaseSeconds: number;
    revisionId: string;
    providers: ConsoleCredentialRefillDemand[];
    recentTasks: ConsoleCredentialRefillTask[];
  };
};

export type ConsoleCredentialRefillRequestResponse = {
  task: ConsoleCredentialRefillTask;
  created: boolean;
};

export type ConsoleCredentialProbeStatus = "passed" | "failed" | "unsupported";

export type ConsoleCredentialProbeResult = {
  credentialId: string;
  providerId: string;
  status: ConsoleCredentialProbeStatus;
  message: string;
  checkedAt: string;
};

export type ConsoleCredentialProbeResponse = {
  result: ConsoleCredentialProbeResult;
};

export type ConsoleGeminiAuthFamily =
  | "gemini-canvas"
  | "gemini-canvas-chat"
  | "gemini-business"
  | "gemini-web";

export type ConsoleGeminiAuthSessionStatus =
  | "pending"
  | "waiting_user"
  | "succeeded"
  | "failed";

export type ConsoleGeminiAuthSecretEdit = {
  field: "api_key" | "auth_token";
  operation: "replace";
  value: string;
};

export type ConsoleGeminiGeneratedCredentialDraft = {
  providerId: string;
  credential: Record<string, unknown>;
  secretEdits: ConsoleGeminiAuthSecretEdit[];
};

export type ConsoleGeminiAuthSession = {
  id: string;
  targetFamily: ConsoleGeminiAuthFamily;
  providerId: string;
  status: ConsoleGeminiAuthSessionStatus;
  message: string;
  createdAt: string;
  updatedAt: string;
  generatedDrafts: ConsoleGeminiGeneratedCredentialDraft[];
};

export type ConsoleGeminiAuthSessionRequest = {
  targetFamily: ConsoleGeminiAuthFamily;
  providerId: string;
  accountLabel?: string;
};

export type ConsoleGeminiAuthSessionResponse = {
  session: ConsoleGeminiAuthSession;
};

export type ConsoleSecretPatch = {
  path: string;
  operation: "keep" | "replace" | "clear";
  value?: string;
};

export type ConsoleEvent = {
  id: string;
  kind: string;
  timestamp: string;
  message?: string;
  data: Record<string, unknown>;
};

export type PublicHealth = {
  status: string;
  [key: string]: unknown;
};

export type PublicReadiness = {
  ready?: boolean;
  status?: string;
  [key: string]: unknown;
};

export type PublicModel = {
  id: string;
  object?: string;
  created?: number;
  owned_by?: string;
  ownedBy?: string;
  [key: string]: unknown;
};

export type PublicModelList = {
  object?: string;
  data: PublicModel[];
  [key: string]: unknown;
};
