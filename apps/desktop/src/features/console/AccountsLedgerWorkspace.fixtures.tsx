import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, vi } from "vitest";

import { NeuroTooltipProvider } from "../../components/ActionTooltip";
import type {
  ConsoleCredentialPoolAutomationProvider,
  ConsoleCredentialRefillDemand,
} from "../../api/contracts";
import {
  AccountsLedgerWorkspace,
  type AccountsLedgerPilotAccount,
  type AccountsLedgerPilotSection,
  type AccountsLedgerWorkspaceProps,
} from "./AccountsLedgerWorkspace";

export function renderWorkspace(props: AccountsLedgerWorkspaceProps) {
  return render(
    <NeuroTooltipProvider>
      <AccountsLedgerWorkspace {...props} />
    </NeuroTooltipProvider>,
  );
}

export function pilotAccount(
  overrides: Partial<AccountsLedgerPilotAccount> = {},
): AccountsLedgerPilotAccount {
  return {
    accountId: "acct-1",
    providerId: "managed-provider",
    displayName: "Account 1",
    mode: "credential",
    enabled: true,
    logicalLabels: [],
    logicalGroupIds: [],
    capacityLabel: "2 / 4",
    statusLabel: "正常",
    dispatchEnabled: true,
    dispatchEditable: true,
    previewOnly: false,
    usageWindowBadges: ["12 req"],
    recentUseLabel: "2 分钟前",
    verificationStatus: "verified",
    verificationFamilies: [],
    verificationCheckedAt: null,
    verificationEvidenceRef: null,
    verificationNote: "",
    ...overrides,
  };
}

export function pilotSection(
  overrides: Partial<AccountsLedgerPilotSection> = {},
): AccountsLedgerPilotSection {
  return {
    providerId: "managed-provider",
    providerIds: ["managed-provider"],
    providerLabel: "Managed OpenAI",
    vendorLabel: "OpenAI",
    providerPreset: "openai",
    adapter: null,
    protocolProfile: null,
    hostLabel: "api.openai.com",
    defaultAccountId: null,
    manualAddFamily: null,
    hasExplicitAccounts: true,
    poolTargetSize: 30,
    autoRefillEnabled: false,
    autoPruneEnabled: false,
    permanentDeleteEnabled: false,
    supportsIdentityCategories: false,
    identityCategories: [],
    directAccounts: [pilotAccount()],
    ...overrides,
  };
}

export function automationProvider(): ConsoleCredentialPoolAutomationProvider {
  return {
    providerId: "managed-provider",
    providerLabel: "Managed OpenAI",
    targetSize: 30,
    credentialCount: 1,
    activeCredentialCount: 1,
    autoRefillEnabled: false,
    autoPruneEnabled: false,
    permanentDeleteEnabled: false,
    driverId: "managed-driver",
    driverMode: "http",
    driverConfigured: true,
    state: "idle",
    lastRunAt: null,
    nextRunAt: null,
    lastAction: null,
    createdCount: 0,
    prunedCount: 0,
    message: null,
    revisionId: null,
  };
}

export function refillDemand(): ConsoleCredentialRefillDemand {
  return {
    providerId: "managed-provider",
    providerLabel: "Managed OpenAI",
    targetSize: 30,
    credentialCount: 1,
    activeCredentialCount: 1,
    deficit: 29,
    needsRefill: true,
    autoRefillEnabled: false,
    directDriverConfigured: true,
    notificationEnabled: true,
    inquiryEnabled: true,
    userRequestEnabled: true,
    outstandingTaskId: null,
    outstandingTaskState: null,
    notificationApi:
      "/v1/internal/gateway/credential-pool-refill/providers/managed-provider/tasks/claim",
    inquiryApi:
      "/v1/internal/gateway/credential-pool-refill/providers/managed-provider",
    credentialStoragePath: "C:\\gateway\\credentials\\managed-provider",
    storagePasswordConfigured: false,
    archiveStoragePath: "C:\\gateway\\credentials\\_archive\\managed-provider",
    archivedCredentialCount: 3,
    permanentDeleteEnabled: false,
    revisionId: "r1-test",
  };
}

export function workspaceProps(
  overrides: Partial<AccountsLedgerWorkspaceProps> = {},
): AccountsLedgerWorkspaceProps {
  return {
    t: (zh, _en) => zh,
    editorLocked: false,
    totalAccounts: 1,
    visibleCount: 1,
    rows: [],
    groupOptions: [],
    pilotSections: [pilotSection()],
    automationByProvider: new Map([["managed-provider", automationProvider()]]),
    pruneBusyProviderId: null,
    refillByProvider: new Map([["managed-provider", refillDemand()]]),
    refillBusyProviderId: null,
    archivePurgeBusyProviderId: null,
    onBackToEditor: vi.fn(),
    onAddIdentityCategory: vi.fn(),
    onOpenGeminiManualAdd: vi.fn(),
    onEdit: vi.fn(),
    onRemove: vi.fn(),
    onAddExplicit: vi.fn(),
    onToggleDispatch: vi.fn(),
    onUpdatePoolTargetSize: vi.fn(),
    onToggleAutoRefill: vi.fn(),
    onToggleAutoPrune: vi.fn(),
    onUpdateProviderPoolTargetSize: vi.fn(),
    onToggleProviderAutoRefill: vi.fn(),
    onToggleProviderAutoPrune: vi.fn(),
    onToggleProviderPermanentDelete: vi.fn(),
    onUpdateProviderStoragePassword: vi.fn(() => true),
    onRequestProviderRefill: vi.fn(),
    onPruneProviderCredentials: vi.fn(),
    onPurgeProviderArchive: vi.fn(),
    onSetAccountGroup: vi.fn(),
    onOpenProbe: vi.fn(),
    onOpenProviderProbe: vi.fn(),
    onRemoveProvider: vi.fn(),
    onOpenProviderSchedule: vi.fn(),
    modelMappingCountByProvider: new Map<string, number>(),
    onOpenModelMapping: vi.fn(),
    onOpenStats: vi.fn(),
    onDuplicate: vi.fn(),
    ...overrides,
  };
}

export function card(providerId: string) {
  const element = document.querySelector(`[data-provider-card="${providerId}"]`);
  expect(element).not.toBeNull();
  return element as HTMLElement;
}
