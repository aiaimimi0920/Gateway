import type { ConsoleProviderQuota } from "../../api/contracts";
import type { ProviderSuccessWindow } from "./providerCardMetrics";
import type { CardModelTraffic } from "./cardModelTraffic";

export type TranslateFn = (zh: string, en: string) => string;

export type AccountCardGroupOption = {
  value: string;
  label: string;
};

/** Shared account shape used by every workspace that renders credential cards. */
export type AccountsLedgerPilotAccount = {
  accountId: string;
  providerId: string;
  displayName: string;
  mode: "credential" | "provider-default";
  enabled: boolean;
  logicalLabels: string[];
  supportedModels?: readonly string[];
  discoverySupported?: boolean;
  modelTraffic?: CardModelTraffic;
  logicalGroupIds?: string[];
  libraryName?: string;
  capacityLabel: string;
  statusLabel: string;
  dispatchEnabled: boolean;
  dispatchEditable: boolean;
  previewOnly: boolean;
  usageWindowBadges: string[];
  recentUseLabel: string;
  verificationStatus: "verified" | "failed" | "blocked" | "not-tested";
  verificationFamilies: string[];
  verificationCheckedAt: string | null;
  verificationEvidenceRef: string | null;
  verificationNote: string;
  concurrencyUsed?: number | null;
  concurrencyTotal?: number | null;
  requestCount?: number | null;
  upstreamCost?: number | null;
  userCost?: number | null;
  successWindows?: ProviderSuccessWindow[] | null;
  quota?: ConsoleProviderQuota | null;
  quotaRemainingUsd?: number | null;
  scheduledProbeEnabled?: boolean;
  scheduledProbeIntervalMinutes?: number;
};

export type QuotaDisplayWindow = {
  key: string;
  label: string;
  remainingRatio: number | null;
  resetAt: string | null;
  accountCount?: number;
};

export type AccountCardActionId = "probe" | "duplicate" | "model-mapping" | "discover";

export type AccountCardAction = (
  action: AccountCardActionId,
  providerId: string,
  account: AccountsLedgerPilotAccount,
) => void;

export type PendingCredentialRemoval = {
  providerId: string;
  accountId: string;
  displayName: string;
};

export type ProviderAccountCardHandlers = {
  onEdit: (providerId: string, accountId: string) => void;
  onRequestRemoval: (providerId: string, accountId: string, displayName: string) => void;
  onAddExplicit: (providerId: string) => void;
  onToggleDispatch: (providerId: string, accountId: string, nextEnabled: boolean) => void;
  onSetAccountGroup: (accountId: string, groupId: string) => void;
  onOpenStats: (providerId: string, account: AccountsLedgerPilotAccount) => void;
};
