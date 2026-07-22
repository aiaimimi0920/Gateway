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
