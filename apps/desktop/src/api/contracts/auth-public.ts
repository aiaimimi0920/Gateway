// Gemini authentication, console event, and public endpoint contracts.

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
