import { render } from "@testing-library/react";
import { vi } from "vitest";

import { NeuroTooltipProvider } from "../../components/ActionTooltip";
import type { CredentialGroupScopeMember } from "./accountManagementViewModel";
import { CredentialGroupsWorkspace, type CredentialGroupsWorkspaceProps } from "./CredentialGroupsWorkspace";
import type { AccountsLedgerPilotAccount } from "./ProviderAccountCard";

/** Shared fixture data keeps the behavior suites focused on their contracts. */
const openaiScopeMember: CredentialGroupScopeMember = {
  accountId: "acct-1",
  providerId: "managed-openai",
  providerLabel: "Managed OpenAI",
  enabled: true,
  metrics: {
    concurrencyUsed: 3,
    concurrencyTotal: 12,
    upstreamCost: 4.5,
    userCost: 9.25,
    usageWindowBadges: ["1,240 req"],
    successWindows: [{ label: "10:00", success: 90, requests: 100 }],
  },
};

const anthropicScopeMember: CredentialGroupScopeMember = {
  accountId: "acct-2",
  providerId: "alt-anthropic",
  providerLabel: "Alt Anthropic",
  enabled: false,
  metrics: {
    concurrencyUsed: 1,
    concurrencyTotal: 4,
    upstreamCost: 0.5,
    userCost: 1.75,
    usageWindowBadges: ["60 req"],
    successWindows: [{ label: "10:00", success: 30, requests: 60 }],
  },
};

export function renderWorkspace(props: CredentialGroupsWorkspaceProps) {
  return render(
    <NeuroTooltipProvider>
      <CredentialGroupsWorkspace {...props} />
    </NeuroTooltipProvider>,
  );
}

export function emptyWorkspaceProps(
  overrides: Partial<CredentialGroupsWorkspaceProps> = {},
): CredentialGroupsWorkspaceProps {
  return {
    t: (_zh, en) => en,
    editorLocked: false,
    groups: [],
    selectedGroupRowId: null,
    selectedGroup: null,
    selectedGroupIdInvalid: false,
    selectedGroupBillingInvalid: false,
    selectedGroupMembers: [],
    memberCandidates: [],
    memberQuery: "",
    memberMode: "all",
    onSelectGroup: vi.fn(),
    onAddGroup: vi.fn(),
    onUpdateField: vi.fn(),
    onToggleEnabled: vi.fn(),
    onRemoveGroup: vi.fn(),
    onMemberQueryChange: vi.fn(),
    onMemberModeChange: vi.fn(),
    onToggleMember: vi.fn(),
    ...overrides,
  };
}

export function populatedWorkspaceProps(
  overrides: Partial<CredentialGroupsWorkspaceProps> = {},
): CredentialGroupsWorkspaceProps {
  return emptyWorkspaceProps({
    groups: [
      {
        rowId: "row-premium",
        groupId: "premium",
        name: "Premium",
        description: "High-priority routing entitlements",
        billingMultiplier: "1.5",
        enabled: true,
        notes: "",
        memberCount: 1,
        providerLabels: ["Managed OpenAI"],
        modelLabels: ["gpt-5"],
        providerScopes: [
          {
            providerId: "alt-anthropic",
            providerLabel: "Alt Anthropic",
            accountCount: 1,
            providerAccountCount: 1,
            modelCount: 2,
          },
          {
            providerId: "managed-openai",
            providerLabel: "Managed OpenAI",
            accountCount: 1,
            providerAccountCount: 1,
            modelCount: 1,
          },
        ],
        modelScopes: [
          { model: "claude-4.5", members: [anthropicScopeMember], providerMetrics: [] },
          {
            model: "gpt-5",
            members: [openaiScopeMember, anthropicScopeMember],
            providerMetrics: [],
          },
        ],
        metrics: {
          concurrency: { used: 3, total: 12 },
          upstreamCost: 4.5,
          platformRevenue: 9.25,
          requests: 1240,
          successWindows: [{ label: "10:00", success: 90, requests: 100, rate: 0.9 }],
          successSuccessCount: 90,
          successRequestCount: 100,
          successRate: 0.9,
        },
      },
    ],
    selectedGroupRowId: "row-premium",
    selectedGroup: {
      id: "row-premium",
      groupId: "premium",
      name: "Premium",
      description: "High-priority routing entitlements",
      billingMultiplier: "1.5",
      enabled: true,
      notes: "",
      providerCredentialIds: ["acct-1"],
    },
    selectedGroupMembers: [
      {
        accountId: "acct-1",
        displayName: "Account One",
        providerId: "managed-openai",
        providerLabel: "Managed OpenAI",
        vendorLabel: "OpenAI",
        mode: "credential",
        enabled: true,
        groupIds: ["premium"],
        groupLabels: ["Premium"],
        selected: true,
        searchText: "account one managed openai premium",
      },
    ],
    memberCandidates: [
      {
        accountId: "acct-1",
        displayName: "Account One",
        providerId: "managed-openai",
        providerLabel: "Managed OpenAI",
        vendorLabel: "OpenAI",
        mode: "credential",
        enabled: true,
        groupIds: ["premium"],
        groupLabels: ["Premium"],
        selected: true,
        searchText: "account one managed openai premium",
      },
      {
        accountId: "acct-2",
        displayName: "Account Two",
        providerId: "managed-openai",
        providerLabel: "Managed OpenAI",
        vendorLabel: "OpenAI",
        mode: "credential",
        enabled: true,
        groupIds: [],
        groupLabels: [],
        selected: false,
        searchText: "account two managed openai",
      },
    ],
    ...overrides,
  });
}

export const pilotAccount: AccountsLedgerPilotAccount = {
  accountId: "acct-1",
  providerId: "managed-openai",
  displayName: "Account One",
  mode: "credential",
  enabled: true,
  logicalLabels: ["Managed OpenAI"],
  capacityLabel: "3/12",
  statusLabel: "Ready",
  dispatchEnabled: true,
  dispatchEditable: true,
  previewOnly: false,
  usageWindowBadges: ["120 req"],
  recentUseLabel: "10:00",
  verificationStatus: "verified",
  verificationFamilies: ["chat"],
  verificationCheckedAt: "2026-08-20T10:00:00Z",
  verificationEvidenceRef: null,
  verificationNote: "",
  concurrencyUsed: 3,
  concurrencyTotal: 12,
};

export function accountCardBridge(
  overrides: Partial<NonNullable<CredentialGroupsWorkspaceProps["accountCards"]>> = {},
): NonNullable<CredentialGroupsWorkspaceProps["accountCards"]> {
  return {
    accountsById: new Map([[pilotAccount.accountId, pilotAccount]]),
    groupOptions: [{ value: "premium", label: "Premium" }],
    handlers: {
      onEdit: vi.fn(),
      onRequestRemoval: vi.fn(),
      onAddExplicit: vi.fn(),
      onToggleDispatch: vi.fn(),
      onSetAccountGroup: vi.fn(),
      onOpenStats: vi.fn(),
    },
    onMenuAction: vi.fn(),
    ...overrides,
  };
}
