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
