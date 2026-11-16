import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { accountCardBridge, emptyWorkspaceProps, populatedWorkspaceProps, renderWorkspace } from "./CredentialGroupsWorkspace.fixtures";

// Cover the current directory, not the retired card faces; mutation contracts live alongside this suite.
describe("CredentialGroupsWorkspace directory", () => {
  it("offers creation when the workspace is empty", async () => {
    const user = userEvent.setup(), onAddGroup = vi.fn();
    renderWorkspace(emptyWorkspaceProps({ onAddGroup }));
    expect(screen.getByText("No groups yet")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Create first group" }));
    expect(onAddGroup).toHaveBeenCalledOnce();
  });
  it("disables empty-state creation while locked", () => {
    renderWorkspace(emptyWorkspaceProps({ editorLocked: true }));
    expect(screen.getByRole("button", { name: "Create first group" })).toBeDisabled();
  });
  it("shows group identity, billing and members without repeating shell actions", () => {
    renderWorkspace(populatedWorkspaceProps({ notice: <div role="status">Draft notice</div> }));
    const nav = within(screen.getByRole("navigation", { name: "Entitlement groups" }));
    expect(nav.getByRole("button", { name: /Premium/ })).toHaveAttribute("aria-current", "true");
    expect(screen.getByRole("heading", { name: "Premium" })).toBeVisible();
    expect(screen.getByText(/Billing multiplier 1.5×/)).toBeVisible();
    const members = within(screen.getByRole("table", { name: "Group accounts" }));
    expect(members.getByText("Account One")).toBeVisible();
    expect(members.queryByText("Account Two")).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Draft notice");
    expect(screen.queryByRole("button", { name: "Add group" })).not.toBeInTheDocument();
  });
  it("opens the editor and restores focus on Escape and Close", async () => {
    const user = userEvent.setup();
    renderWorkspace(populatedWorkspaceProps());
    const edit = screen.getByRole("button", { name: "Edit group" });
    await user.click(edit);
    const dialog = screen.getByRole("dialog", { name: "Edit entitlement group" });
    expect(within(dialog).getByLabelText("Group ID")).toHaveValue("premium");
    expect(within(dialog).getByLabelText("Group name")).toHaveFocus();
    expect(within(dialog).getByRole("region", { name: "Member management" })).toBeVisible();
    expect(document.body).toHaveAttribute("data-scroll-locked");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(edit).toHaveFocus());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(edit);
    await user.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(edit).toHaveFocus());
  });
  it("allows reading locked groups without mutations", async () => {
    const user = userEvent.setup(), props = populatedWorkspaceProps({ editorLocked: true });
    renderWorkspace(props);
    for (const name of ["Edit group", "Delete group"]) {
      const action = screen.getByRole("button", { name });
      expect(action).toBeDisabled();
      await user.click(action);
    }
    expect(screen.getByRole("switch", { name: "Premium enabled" })).toBeDisabled();
    expect(props.onRemoveGroup).not.toHaveBeenCalled();
    expect(props.onUpdateField).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
  it("manages resolved accounts with stable identities", async () => {
    const user = userEvent.setup(), bridge = accountCardBridge();
    renderWorkspace(populatedWorkspaceProps({ accountCards: bridge }));
    await user.click(screen.getByRole("button", { name: "Manage account" }));
    const dialog = screen.getByRole("dialog", { name: "Account management" });
    expect(dialog.querySelector(".nt-provider-account-card")).not.toBeNull();
    await user.click(within(dialog).getByRole("button", { name: "Edit account Account One" }));
    expect(bridge.handlers.onEdit).toHaveBeenCalledWith("managed-openai", "acct-1");
  });
  it("retains unresolved members without offering management for missing accounts", () => {
    renderWorkspace(populatedWorkspaceProps({ accountCards: accountCardBridge({ accountsById: new Map() }) }));
    expect(within(screen.getByRole("table", { name: "Group accounts" })).getByText("Account One")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Manage account" })).not.toBeInTheDocument();
  });
  it("filters members by account or provider without losing the unfiltered list", async () => {
    const user = userEvent.setup();
    renderWorkspace(populatedWorkspaceProps());
    const search = screen.getByRole("textbox", { name: "Search group content" });
    await user.type(search, "missing");
    expect(screen.getByText("No matching content")).toBeVisible();
    expect(screen.queryByText("Account One")).not.toBeInTheDocument();
    await user.clear(search);
    await user.type(search, "managed openai");
    expect(within(screen.getByRole("table", { name: "Group accounts" })).getByText("Account One")).toBeVisible();
  });
  it("shows one model row with all associated providers and exact account count", async () => {
    const user = userEvent.setup();
    renderWorkspace(populatedWorkspaceProps());
    await user.click(screen.getByRole("radio", { name: "Models" }));
    const table = within(screen.getByRole("table", { name: "Group models" }));
    const row = table.getByRole("row", { name: /gpt-5/ });
    expect(within(row).getByRole("cell", { name: "2" })).toBeVisible();
    expect(row).toHaveTextContent("Managed OpenAI");
    expect(row).toHaveTextContent("Alt Anthropic");
    expect(table.getAllByText("gpt-5")).toHaveLength(1);
    await user.type(screen.getByRole("textbox", { name: "Search group content" }), "claude");
    expect(table.getByText("claude-4.5")).toBeVisible();
    expect(table.queryByText("gpt-5")).not.toBeInTheDocument();
  });
  it("paginates members and resets the page after filtering", async () => {
    const user = userEvent.setup(), base = populatedWorkspaceProps();
    const members = Array.from({ length: 21 }, (_, index) => ({ ...base.selectedGroupMembers[0], accountId: "id-" + index, displayName: "Member " + index }));
    renderWorkspace(populatedWorkspaceProps({ selectedGroupMembers: members }));
    const pager = within(screen.getByRole("navigation", { name: "Group content pages" }));
    expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(21);
    await user.click(pager.getByRole("button", { name: "Next page" }));
    expect(screen.getByText("Member 20")).toBeVisible();
    expect(screen.queryByText("Member 0")).not.toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: "Search group content" }), "Member 0");
    expect(screen.getByText("Member 0")).toBeVisible();
    expect(screen.queryByRole("navigation", { name: "Group content pages" })).not.toBeInTheDocument();
    expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(2);
  });
});
