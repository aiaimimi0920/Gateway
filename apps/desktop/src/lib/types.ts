export type SectionId = "launcher" | "config" | "status" | "models" | "api-test" | "logs";

export type GatewayUiRuntimeInfo = {
  appName: string;
  themeFamily: string;
  gatewayMode: string;
};

export type GatewayEnvEntry = {
  key: string;
  value: string;
};

export type GatewayRuntimeRole = "splitter" | "worker" | "standalone";

export type GatewayProfile = {
  name: string;
  runtimeRole: GatewayRuntimeRole;
  gatewayManagementToken?: string | null;
  port: number;
  gatewayRedisUrl: string;
  gatewayDatabaseUrl?: string | null;
  gatewayRoutesFile?: string | null;
  logLevel?: string | null;
  workingDirectory?: string | null;
  extraEnv: GatewayEnvEntry[];
};

export type GatewayProfileTransferPayload = {
  schemaVersion: 1;
  kind: "gateway-ui-profile";
  exportedAt: string;
  profile: GatewayProfile;
};

export type GatewayProfilePathCheckItem = {
  configuredPath?: string | null;
  resolvedPath: string;
  expectedKind: "file" | "directory" | string;
  exists: boolean;
  isFile: boolean;
  isDir: boolean;
  ok: boolean;
  message: string;
};

export type GatewayProfilePathCheck = {
  workingDirectory: GatewayProfilePathCheckItem;
  gatewayRoutesFile: GatewayProfilePathCheckItem;
  sidecar: GatewayProfilePathCheckItem;
  redis: GatewayDependencyCheckItem;
  database: GatewayDependencyCheckItem;
  preflightOk: boolean;
  preflightMessages: string[];
};

export type GatewayDependencyCheckItem = {
  name: string;
  required: boolean;
  configured: boolean;
  ok: boolean;
  message: string;
};

export type GatewayProfileTemplate = {
  id: string;
  title: string;
  description: string;
  profile: GatewayProfile;
};

export type GatewayOnboardingStep = {
  id: string;
  title: string;
  description: string;
  status: "done" | "pending" | "warning";
};

export type GatewayProfileValidationField =
  | "name"
  | "runtimeRole"
  | "gatewayManagementToken"
  | "port"
  | "gatewayRedisUrl"
  | "gatewayDatabaseUrl"
  | "gatewayRoutesFile"
  | "logLevel"
  | "workingDirectory"
  | "extraEnv";

export type GatewayProfileValidation = {
  ok: boolean;
  errors: string[];
  warnings: string[];
  fieldErrors: Partial<Record<GatewayProfileValidationField, string>>;
  extraEnvErrors: Record<number, string>;
};

export type GatewayProcessSnapshot = {
  running: boolean;
  pid?: number | null;
  port?: number | null;
  profileName?: string | null;
  logPath?: string | null;
  startedAt?: string | null;
  startupState?: string | null;
  shutdownState?: string | null;
  lastError?: string | null;
  recentLogLines: string[];
};

export type GatewayLogTail = {
  path?: string | null;
  lines: string[];
};

export type GatewayHttpProbe<TData = unknown> = {
  ok: boolean;
  status: number;
  durationMs: number;
  data?: TData;
  error?: string;
};

export type GatewayHealthResponse = {
  status?: string;
  [key: string]: unknown;
};

export type GatewayReadyResponse = {
  ready?: boolean;
  status?: string;
  checks?: unknown;
  [key: string]: unknown;
};

export type GatewayModelObject = {
  id: string;
  object?: string;
  created?: number;
  owned_by?: string;
  ownedBy?: string;
  [key: string]: unknown;
};

export type GatewayModelListResponse = {
  object?: string;
  data?: GatewayModelObject[];
  [key: string]: unknown;
};

export type GatewayApiTestInput = {
  apiKey: string;
  model: string;
  message: string;
};

export type GatewayApiTestResult = GatewayHttpProbe<unknown> & {
  endpoint: string;
  requestedAt: string;
  profileName: string;
  baseUrl: string;
  model: string;
};

export type GatewayDesktopNotice = {
  tone: "info" | "success" | "warning" | "danger";
  message: string;
};

export type GatewayDesktopState = {
  runtimeInfo?: GatewayUiRuntimeInfo;
  profileNames: string[];
  selectedProfileName: string;
  draftProfile: GatewayProfile;
  processSnapshot: GatewayProcessSnapshot;
  healthProbe?: GatewayHttpProbe<GatewayHealthResponse>;
  readyProbe?: GatewayHttpProbe<GatewayReadyResponse>;
  modelsProbe?: GatewayHttpProbe<GatewayModelListResponse>;
  logTail: GatewayLogTail;
  diagnosticsReportText?: string;
  profileTransferText: string;
  importProfileText: string;
  pathCheck?: GatewayProfilePathCheck;
  onboardingSteps: GatewayOnboardingStep[];
  hasUnsavedProfileChanges: boolean;
  startWillSaveDraftProfile: boolean;
  notice?: GatewayDesktopNotice;
  busy: boolean;
  apiTestInput: GatewayApiTestInput;
  apiTestResult?: GatewayApiTestResult;
  profileValidation: GatewayProfileValidation;
  canSaveProfile: boolean;
  canStartGateway: boolean;
  baseUrl: string;
  isTauriAvailable: boolean;
  refreshAll: () => Promise<void>;
  reloadProfiles: (preferredProfileName?: string) => Promise<void>;
  selectProfile: (name: string) => Promise<void>;
  updateDraftProfile: (profile: GatewayProfile) => void;
  saveDraftProfile: () => Promise<void>;
  deleteSelectedProfile: () => Promise<void>;
  startGateway: () => Promise<void>;
  stopGateway: () => Promise<void>;
  refreshRuntimeSignals: () => Promise<void>;
  refreshModels: () => Promise<void>;
  refreshLogs: () => Promise<void>;
  openLogDirectory: () => Promise<void>;
  copyCurrentLogPath: () => Promise<void>;
  copyDiagnostics: () => Promise<void>;
  exportDraftProfile: () => Promise<void>;
  importDraftProfile: () => Promise<void>;
  updateImportProfileText: (value: string) => void;
  checkProfilePaths: () => Promise<void>;
  applyProfileTemplate: (templateId: string) => void;
  updateApiTestInput: (input: GatewayApiTestInput) => void;
  runApiTest: () => Promise<void>;
  clearNotice: () => void;
};
