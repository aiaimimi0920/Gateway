import type { ReactNode } from "react";

import type {
  AccountGroupDraftLike,
  CredentialGroupDirectoryItem,
  GroupMemberCandidate,
} from "./accountManagementViewModel";
import type {
  AccountCardGroupOption,
  AccountCardMenuActionId,
  AccountCardMenuItem,
  AccountsLedgerPilotAccount,
  ProviderAccountCardHandlers,
} from "./ProviderAccountCard";

export type TranslateFn = (zh: string, en: string) => string;

export type EntitlementAccountCardBridge = {
  accountsById: ReadonlyMap<string, AccountsLedgerPilotAccount>;
  groupOptions: AccountCardGroupOption[];
  handlers: ProviderAccountCardHandlers;
  menuItems?: readonly AccountCardMenuItem[];
  onMenuAction: (
    action: AccountCardMenuActionId,
    providerId: string,
    account: AccountsLedgerPilotAccount,
  ) => void;
};

export type CredentialGroupsWorkspaceProps = {
  t: TranslateFn;
  notice?: ReactNode;
  editorLocked: boolean;
  groups: CredentialGroupDirectoryItem[];
  selectedGroupRowId: string | null;
  selectedGroup: AccountGroupDraftLike | null;
  selectedGroupIdInvalid: boolean;
  selectedGroupBillingInvalid: boolean;
  selectedGroupMembers: GroupMemberCandidate[];
  memberCandidates: GroupMemberCandidate[];
  memberQuery: string;
  memberMode: "all" | "members" | "ungrouped";
  accountCards?: EntitlementAccountCardBridge;
  onSelectGroup: (rowId: string) => void;
  onAddGroup: () => void;
  onUpdateField: (
    rowId: string,
    field: "groupId" | "name" | "description" | "billingMultiplier" | "notes",
    value: string,
  ) => void;
  onToggleEnabled: (rowId: string, enabled: boolean) => void;
  onRemoveGroup: (rowId: string) => void;
  onMemberQueryChange: (value: string) => void;
  onMemberModeChange: (value: "all" | "members" | "ungrouped") => void;
  onToggleMember: (rowId: string, accountId: string) => void;
};
