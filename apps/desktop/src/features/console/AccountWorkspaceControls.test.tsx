import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { CredentialGroupEditor } from "./CredentialGroupEditor";
import { CredentialGroupMembersPanel } from "./CredentialGroupMembersPanel";
import { CredentialGroupEditDialog } from "./CredentialGroupEditDialog";
import { populatedWorkspaceProps } from "./CredentialGroupsWorkspace.fixtures";
import { AccountsLedgerWorkspace } from "./AccountsLedgerWorkspace";
import { workspaceProps } from "./AccountsLedgerWorkspace.fixtures";

describe("account workspace standard controls", () => {
  it("preserves the settings dialog and disables editing during a background lock", () => {
    const props = workspaceProps();
    const { rerender } = render(<NeuroProvider><AccountsLedgerWorkspace {...props} /></NeuroProvider>);
    fireEvent.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /^Managed OpenAI/ }));
    fireEvent.click(screen.getByRole("button", { name: "服务商设置" }));
    const opener = screen.getByRole("button", { name: "编辑 Managed OpenAI 最大可用池" });
    rerender(<NeuroProvider><AccountsLedgerWorkspace {...props} editorLocked /></NeuroProvider>);
    expect(opener).toBeDisabled();
    expect(screen.getByRole("button", { name: "编辑 Managed OpenAI 最大可用池" })).toBe(opener);
    expect(screen.getByRole("dialog", { name: "服务商设置" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "关闭" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("keeps member row, operation node, and focus across selection and refreshed draft IDs", () => {
    const props = populatedWorkspaceProps();
    const toggle = vi.fn();
    const view = (id: string, selected: boolean) => <NeuroProvider><CredentialGroupMembersPanel
      t={props.t} group={{ ...props.selectedGroup!, id }} editorLocked={false}
      candidates={props.memberCandidates.map((candidate, index) => index ? candidate : { ...candidate, selected })}
      query="" mode="all" onQueryChange={vi.fn()} onModeChange={vi.fn()} onToggleMember={toggle} />
    </NeuroProvider>;
    const { rerender } = render(view("initial-row", false));
    const button = screen.getByRole("button", { name: "Add Account One" });
    const row = button.closest("tr");
    act(() => button.focus());
    fireEvent.click(button);
    expect(toggle).toHaveBeenLastCalledWith("initial-row", "acct-1");
    rerender(view("saved-row", true));
    expect(screen.getByRole("button", { name: "Remove Account One" })).toBe(button);
    expect(button.closest("tr")).toBe(row);
    expect(button).toHaveFocus();
    fireEvent.click(button);
    expect(toggle).toHaveBeenLastCalledWith("saved-row", "acct-1");
    expect(screen.getByRole("table", { name: "Candidate accounts" })).toHaveClass("n-table");
  });

  it("generates unique field associations even when editors share the same business ID", () => {
    const props = populatedWorkspaceProps();
    const editor = <CredentialGroupEditor t={props.t} group={props.selectedGroup!}
      editorLocked={false} selectedGroupIdInvalid selectedGroupBillingInvalid
      onUpdateField={vi.fn()} onToggleEnabled={vi.fn()} onRemoveGroup={vi.fn()} />;
    render(<NeuroProvider>{editor}{editor}</NeuroProvider>);
    const ids = screen.getAllByLabelText("Group ID");
    expect(ids).toHaveLength(2);
    expect(ids[0].id).not.toBe(ids[1].id);
    for (const input of ids) {
      expect(input).toHaveAccessibleDescription("Group ID is required.");
      expect(input).toHaveAttribute("aria-invalid", "true");
    }
  });

  it("keeps independent save errors visible alongside invalid fields without duplicating the same error", () => {
    const props = populatedWorkspaceProps();
    const view = (error: string, groupIdError?: string) => <NeuroProvider><CredentialGroupEditDialog
      {...props} group={props.selectedGroup!} selectedGroupIdInvalid error={error}
      groupIdError={groupIdError} onClose={vi.fn()} />
    </NeuroProvider>;
    const { rerender } = render(view("Save rejected"));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("Save rejected")).toBeVisible();
    expect(within(dialog).getByLabelText("Group ID")).toHaveAccessibleDescription("Group ID is required.");
    rerender(view("Duplicate group", "Duplicate group"));
    expect(within(dialog).getAllByText("Duplicate group")).toHaveLength(1);
    expect(within(dialog).getByLabelText("Group ID")).toHaveAccessibleDescription("Duplicate group");
  });
});
