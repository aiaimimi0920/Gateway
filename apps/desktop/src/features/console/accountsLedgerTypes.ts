import type { StorageConnection, StorageSecretChanges, StorageTarget } from "./providerStorageConnection";
import type { ReactNode } from "react";
import type {
  ConsoleCredentialPoolAutomationProvider,
  ConsoleCredentialRefillDemand,
} from "../../api/contracts";
import type { AccountLedgerRow } from "./accountManagementViewModel";
import type { AccountsLedgerPilotAccount } from "./accountCardTypes";
import type { ProviderAggregateMetrics } from "./providerCardMetrics";
import type { CardModelTraffic } from "./cardModelTraffic";

export type TranslateFn = (zh: string, en: string) => string;

type Option = {
  value: string;
  label: string;
};

/** Re-exported so the existing pool call sites keep their import path. */
export type { AccountsLedgerPilotAccount };

export type AccountsLedgerPilotCategory = {
  id: string;
  label: string;
  count: number;
  poolTargetSize: number;
  autoRefillEnabled: boolean;
  autoPruneEnabled: boolean;
  accounts: AccountsLedgerPilotAccount[];
};

export type AccountsLedgerPilotSection = {
  providerId: string;
  providerIds: string[];
  providerLabel: string;
  vendorLabel: string;
  providerPreset: string | null;
  adapter: string | null;
  protocolProfile: string | null;
  hostLabel: string | null;
  defaultAccountId: string | null;
  manualAddFamily:
    | "gemini-canvas"
    | "gemini-canvas-chat"
    | "gemini-business"
    | "gemini-web"
    | null;
  hasExplicitAccounts: boolean;
  poolMinSize?: number;
  credentialStoragePath?: string;
  credentialArchivePath?: string;
  credentialStorageConnection?: StorageConnection;
  credentialArchiveConnection?: StorageConnection;
  poolTargetSize: number;
  autoRefillEnabled: boolean;
  autoPruneEnabled: boolean;
  permanentDeleteEnabled: boolean;
  supportsIdentityCategories: boolean;
  identityCategories: AccountsLedgerPilotCategory[];
  directAccounts: AccountsLedgerPilotAccount[];
  /**
   * Provider-account rollup straight from live telemetry. Preferred over summing
   * the visible account cards for two reasons: the gateway records requests,
   * money and concurrency per provider account only — never per credential — so
   * a pool with several credentials has nothing to put on each card; and summing
   * the *visible* cards would make provider totals shrink while the operator
   * types in the search box.
   */
  telemetry?: ProviderAggregateMetrics | null;
  modelTraffic?: CardModelTraffic;
};

export type AccountsLedgerWorkspaceProps = {
  t: TranslateFn;
  notice?: ReactNode;
  editorLocked: boolean;
  discardLifecycleDrafts?: boolean;
  lifecycleActionsLocked?: boolean;
  totalAccounts: number;
  visibleCount: number;
  rows: AccountLedgerRow[];
  groupOptions: Option[];
  pilotSections: AccountsLedgerPilotSection[];
  automationByProvider: ReadonlyMap<string, ConsoleCredentialPoolAutomationProvider>;
  pruneBusyProviderId: string | null;
  refillByProvider: ReadonlyMap<string, ConsoleCredentialRefillDemand>;
  refillBusyProviderId: string | null;
  archivePurgeBusyProviderId: string | null;
  /** Retained for existing test fixtures; the legacy editor button is no longer rendered. */
  onBackToEditor?: () => void;
  onAddIdentityCategory: (providerId: string) => void;
  onOpenGeminiManualAdd: (
    targetFamily:
      | "gemini-canvas"
      | "gemini-canvas-chat"
      | "gemini-business"
      | "gemini-web",
    providerId: string,
  ) => void;
  onEdit: (providerId: string, accountId: string) => void;
  onRemove: (providerId: string, accountId: string, displayName: string) => void;
  onAddExplicit: (providerId: string) => void;
  onToggleDispatch: (providerId: string, accountId: string, nextEnabled: boolean) => void;
  onUpdatePoolTargetSize: (providerId: string, categoryId: string, nextTargetSize: number) => void;
  onToggleAutoRefill: (providerId: string, categoryId: string, nextEnabled: boolean) => void;
  onToggleAutoPrune: (providerId: string, categoryId: string, nextEnabled: boolean) => void;
  storageSecretFields?: ReadonlyMap<string, readonly string[]>;
  onUpdateProviderStorageConnection?: (providerId: string, target: StorageTarget, connection: StorageConnection, secrets: StorageSecretChanges) => boolean;
  onUpdateProviderPoolMinSize?: (providerId: string, nextMinSize: number) => void;
  onUpdateProviderStoragePath?: (providerId: string, path: string) => boolean;
  onUpdateProviderArchivePath?: (providerId: string, path: string) => boolean;
  onUpdateProviderPoolTargetSize: (providerId: string, nextTargetSize: number) => void;
  onToggleProviderAutoRefill: (providerId: string, nextEnabled: boolean) => void;
  onToggleProviderAutoPrune: (providerId: string, nextEnabled: boolean) => void;
  onToggleProviderPermanentDelete: (providerId: string, nextEnabled: boolean) => void;
  onUpdateProviderStoragePassword: (providerId: string, password: string) => boolean;
  onRequestProviderRefill: (providerId: string) => void;
  onPruneProviderCredentials: (providerId: string) => void;
  onPurgeProviderArchive: (providerId: string) => void;
  onSetAccountGroup: (accountId: string, groupId: string) => void;
  onOpenProbe: (providerId: string, account: AccountsLedgerPilotAccount) => void;
  onOpenProviderProbe: (section: AccountsLedgerPilotSection) => void;
  onOpenProviderSchedule: (section: AccountsLedgerPilotSection) => void;
  /** How many `model_map` entries each provider carries, for the menu badge. */
  modelMappingCountByProvider: ReadonlyMap<string, number>;
  onOpenModelMapping: (section: AccountsLedgerPilotSection) => void;
  onOpenStats: (providerId: string, account: AccountsLedgerPilotAccount) => void;
  onDuplicate: (providerId: string, account: AccountsLedgerPilotAccount) => void;
};
