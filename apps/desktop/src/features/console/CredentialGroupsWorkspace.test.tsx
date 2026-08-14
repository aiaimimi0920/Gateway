import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { CredentialGroupsWorkspace, type CredentialGroupsWorkspaceProps } from "./CredentialGroupsWorkspace";

function emptyWorkspaceProps(
  overrides: Partial<CredentialGroupsWorkspaceProps> = {},
): CredentialGroupsWorkspaceProps {
  return {
    t: (_zh, en) => en,
    editorLocked: false,
    totalAccounts: 0,
    totalGroups: 0,
    enabledGroupCount: 0,
    ungroupedCount: 0,
    groups: [],
    selectedGroupRowId: null,
    selectedGroup: null,
    selectedGroupIdInvalid: false,
    selectedGroupBillingInvalid: false,
    memberCandidates: [],
    memberQuery: "",
    memberMode: "all",
    onSelectGroup: vi.fn(),
    onAddGroup: vi.fn(),
    onBackToAccounts: vi.fn(),
    onUpdateField: vi.fn(),
    onToggleEnabled: vi.fn(),
    onRemoveGroup: vi.fn(),
    onMemberQueryChange: vi.fn(),
    onMemberModeChange: vi.fn(),
    onToggleMember: vi.fn(),
    ...overrides,
  };
}

describe("CredentialGroupsWorkspace", () => {
  it("offers an in-context action when no groups exist", async () => {
    const onAddGroup = vi.fn();
    const user = userEvent.setup();

    render(<CredentialGroupsWorkspace {...emptyWorkspaceProps({ onAddGroup })} />);

    expect(screen.getByRole("heading", { name: "No groups yet" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Create first group" }));
    expect(onAddGroup).toHaveBeenCalledTimes(1);
  });

  it("disables both creation actions while the editor is locked", () => {
    render(<CredentialGroupsWorkspace {...emptyWorkspaceProps({ editorLocked: true })} />);

    expect(screen.getByRole("button", { name: "Add group" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Create first group" })).toBeDisabled();
  });
});
