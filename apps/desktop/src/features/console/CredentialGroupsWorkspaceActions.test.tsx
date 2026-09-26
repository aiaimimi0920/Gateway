import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { NeuroTooltipProvider } from "../../components/ActionTooltip";
import {
  CredentialGroupsWorkspace,
  type CredentialGroupsWorkspaceProps,
} from "./CredentialGroupsWorkspace";

function workspaceProps(
  overrides: Partial<CredentialGroupsWorkspaceProps> = {},
): CredentialGroupsWorkspaceProps {
  const members = [
    {
      accountId: "acct-1",
      displayName: "Account One",
      providerId: "managed-openai",
      providerLabel: "Managed OpenAI",
      vendorLabel: "OpenAI",
      mode: "credential" as const,
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
      mode: "credential" as const,
      enabled: true,
      groupIds: [],
      groupLabels: [],
      selected: false,
      searchText: "account two managed openai",
    },
  ];

  return {
    t: (_zh, en) => en,
    editorLocked: false,
    groups: [
      {
        rowId: "row-premium",
        groupId: "premium",
        name: "Premium",
        description: "Priority routing",
        billingMultiplier: "1.5",
        enabled: true,
        notes: "Initial note",
        memberCount: 1,
        providerLabels: ["Managed OpenAI"],
        modelLabels: ["gpt-5"],
        providerScopes: [],
        modelScopes: [],
        metrics: null,
      },
    ],
    selectedGroupRowId: "row-premium",
    selectedGroup: {
      id: "row-premium",
      groupId: "premium",
      name: "Premium",
      description: "Priority routing",
      billingMultiplier: "1.5",
      enabled: true,
      notes: "Initial note",
      providerCredentialIds: ["acct-1"],
    },
    selectedGroupIdInvalid: false,
    selectedGroupBillingInvalid: false,
    selectedGroupMembers: [members[0]],
    memberCandidates: members,
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

function renderExpanded(props: CredentialGroupsWorkspaceProps) {
  const view = render(
    <NeuroTooltipProvider>
      <CredentialGroupsWorkspace {...props} />
    </NeuroTooltipProvider>,
  );
  const card = document.querySelector('[data-entitlement-group-card="premium"]') as HTMLElement;
  fireEvent.click(within(card).getByRole("button", { name: "Premium" }));
  return { ...view, card, detail: screen.getByRole("region", { name: "Group detail" }) };
}

describe("CredentialGroupsWorkspace actions", () => {
  it("preserves card actions, editor callbacks, and invalid-field associations", () => {
    const props = workspaceProps({
      selectedGroupIdInvalid: true,
      selectedGroupBillingInvalid: true,
    });
    const { card, detail } = renderExpanded(props);

    fireEvent.click(within(card).getByRole("switch", { name: "Premium enabled state" }));
    fireEvent.click(within(card).getByRole("button", { name: "Edit" }));
    fireEvent.click(within(card).getByRole("button", { name: "Delete" }));
    expect(props.onToggleEnabled).toHaveBeenCalledWith("row-premium", false);
    expect(props.onSelectGroup).toHaveBeenLastCalledWith("row-premium");
    expect(props.onRemoveGroup).toHaveBeenCalledWith("row-premium");

    const fields = [
      ["Group ID", "groupId", "replacement-id"],
      ["Group name", "name", "Replacement"],
      ["Billing multiplier", "billingMultiplier", "2.25"],
      ["Description", "description", "Replacement description"],
      ["Notes", "notes", "Replacement note"],
    ] as const;
    for (const [label, field, value] of fields) {
      fireEvent.change(within(detail).getByLabelText(new RegExp(`^${label}`)), {
        target: { value },
      });
      expect(props.onUpdateField).toHaveBeenLastCalledWith("row-premium", field, value);
    }

    const groupId = within(detail).getByLabelText(/^Group ID/);
    const billing = within(detail).getByLabelText(/^Billing multiplier/);
    expect(groupId).toHaveAttribute("aria-invalid", "true");
    expect(groupId).toHaveAccessibleDescription("Group ID is required.");
    expect(billing).toHaveAttribute("aria-invalid", "true");
    expect(billing).toHaveAccessibleDescription(
      "Group billing multiplier must be a number greater than or equal to 0.",
    );

    fireEvent.click(within(detail).getByRole("checkbox", { name: "Premium enabled state" }));
    fireEvent.click(within(detail).getByRole("button", { name: "Remove group" }));
    expect(props.onToggleEnabled).toHaveBeenLastCalledWith("row-premium", false);
    expect(props.onRemoveGroup).toHaveBeenLastCalledWith("row-premium");
  });

  it("preserves member filtering and add/remove callback identities", async () => {
    const user = userEvent.setup();
    const props = workspaceProps();
    const { detail } = renderExpanded(props);

    fireEvent.change(within(detail).getByLabelText("Filter candidate accounts"), {
      target: { value: "managed" },
    });
    await user.selectOptions(within(detail).getByLabelText("Candidate scope"), "ungrouped");
    await user.click(within(detail).getByRole("button", { name: "Remove Account One" }));
    await user.click(within(detail).getByRole("button", { name: "Add Account Two" }));

    expect(props.onMemberQueryChange).toHaveBeenCalledWith("managed");
    expect(props.onMemberModeChange).toHaveBeenCalledWith("ungrouped");
    expect(props.onToggleMember).toHaveBeenNthCalledWith(1, "row-premium", "acct-1");
    expect(props.onToggleMember).toHaveBeenNthCalledWith(2, "row-premium", "acct-2");
  });

  it("locks every mutating card, editor, and membership control", () => {
    const props = workspaceProps({ editorLocked: true });
    const { card, detail } = renderExpanded(props);

    expect(within(card).getByRole("switch", { name: "Premium enabled state" })).toBeDisabled();
    expect(within(card).getByRole("button", { name: "Edit" })).toBeDisabled();
    expect(within(card).getByRole("button", { name: "Delete" })).toBeDisabled();
    expect(within(detail).getByRole("checkbox", { name: "Premium enabled state" })).toBeDisabled();
    expect(within(detail).getByRole("button", { name: "Remove group" })).toBeDisabled();
    expect(within(detail).getByLabelText("Group ID")).toBeDisabled();
    expect(within(detail).getByLabelText("Group name")).toBeDisabled();
    expect(within(detail).getByLabelText("Billing multiplier")).toBeDisabled();
    expect(within(detail).getByLabelText("Description")).toBeDisabled();
    expect(within(detail).getByLabelText("Notes")).toBeDisabled();
    expect(within(detail).getByRole("button", { name: "Remove Account One" })).toBeDisabled();
    expect(within(detail).getByRole("button", { name: "Add Account Two" })).toBeDisabled();
    expect(within(detail).getByLabelText("Filter candidate accounts")).toBeEnabled();
    expect(within(detail).getByLabelText("Candidate scope")).toBeEnabled();
  });
});
