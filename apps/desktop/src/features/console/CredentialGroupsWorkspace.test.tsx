import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { AccountsLedgerPilotAccount } from "./ProviderAccountCard";
import {
  accountCardBridge,
  emptyWorkspaceProps,
  pilotAccount,
  populatedWorkspaceProps,
  renderWorkspace,
} from "./CredentialGroupsWorkspace.fixtures";

describe("CredentialGroupsWorkspace", () => {
  it("offers an in-context action when no groups exist", async () => {
    const onAddGroup = vi.fn();
    const user = userEvent.setup();

    renderWorkspace(emptyWorkspaceProps({ onAddGroup }));

    expect(screen.getByRole("heading", { name: "No groups yet" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Create first group" }));
    expect(onAddGroup).toHaveBeenCalledTimes(1);
  });

  it("disables the empty-state creation action while the editor is locked", () => {
    renderWorkspace(emptyWorkspaceProps({ editorLocked: true }));

    expect(screen.getByRole("button", { name: "Create first group" })).toBeDisabled();
  });

  it("leaves the page title and its action to the shell header", () => {
    renderWorkspace(populatedWorkspaceProps({ notice: <div role="status">Draft notice</div> }));

    // The shell board header owns "Entitlement groups" plus the "Add group"
    // action now, matching the credential pool, so the workspace itself must not
    // repeat them in a third command bar.
    expect(screen.queryByRole("heading", { name: "Entitlement groups" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add group" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Back to accounts" })).not.toBeInTheDocument();
    expect(screen.getByText("Draft notice")).toBeInTheDocument();
  });

  it("flips an entitlement card without expanding its account library", async () => {
    const user = userEvent.setup();
    renderWorkspace(populatedWorkspaceProps());

    const groupCard = document.querySelector('[data-entitlement-group-card="premium"]') as HTMLElement;
    const front = groupCard.querySelector(".nt-entitlement-group-card__front") as HTMLElement;
    const back = groupCard.querySelector(".nt-entitlement-group-card__back") as HTMLElement;

    expect(groupCard).toHaveAttribute("data-entitlement-group-card-side", "front");
    expect(front).toHaveAttribute("aria-hidden", "false");
    expect(back).toHaveAttribute("aria-hidden", "true");

    await user.click(within(groupCard).getByRole("button", { name: "Flip Premium for details" }));

    expect(groupCard).toHaveAttribute("data-entitlement-group-card-side", "back");
    expect(front).toHaveAttribute("aria-hidden", "true");
    expect(back).toHaveAttribute("aria-hidden", "false");
    expect(within(back).getByText("Model scope")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Premium group accounts" })).not.toBeInTheDocument();

    const flipBack = within(back).getByRole("button", { name: "Flip Premium back to the front" });
    expect(flipBack).toHaveFocus();
    await user.click(flipBack);
    expect(groupCard).toHaveAttribute("data-entitlement-group-card-side", "front");
    expect(within(front).getByRole("button", { name: "Flip Premium for details" })).toHaveFocus();
  });

  it("does not rebuild card availability data for a local flip", async () => {
    const user = userEvent.setup();
    const baseGroup = populatedWorkspaceProps().groups[0];
    const successWindows = baseGroup.metrics?.successWindows ?? [];
    let successWindowReads = 0;
    const metrics = { ...baseGroup.metrics! };
    Object.defineProperty(metrics, "successWindows", {
      configurable: true,
      get: () => {
        successWindowReads += 1;
        return successWindows;
      },
    });
    renderWorkspace(populatedWorkspaceProps({ groups: [{ ...baseGroup, metrics }] }));

    expect(successWindowReads).toBe(1);
    const groupCard = document.querySelector('[data-entitlement-group-card="premium"]') as HTMLElement;
    await user.click(within(groupCard).getByRole("button", { name: "Flip Premium for details" }));

    expect(successWindowReads).toBe(1);
  });

  it("renders entitlement cards and expands the selected group's account cards", async () => {
    const onSelectGroup = vi.fn();
    const user = userEvent.setup();
    renderWorkspace(populatedWorkspaceProps({ onSelectGroup }));

    const groupCard = document.querySelector('[data-entitlement-group-card="premium"]');
    expect(groupCard).not.toBeNull();
    expect(screen.getByRole("region", { name: "Entitlement group cards" })).toContainElement(
      groupCard as HTMLElement,
    );
    const toggle = within(groupCard as HTMLElement).getByRole("button", { name: "Premium" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(within(groupCard as HTMLElement).queryByText("Account One")).not.toBeInTheDocument();

    await user.click(toggle);

    expect(onSelectGroup).toHaveBeenCalledWith("row-premium");
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    const accountLibrary = screen.getByRole("region", { name: "Premium group accounts" });
    expect(accountLibrary).toHaveClass("nt-entitlement-group-card__accounts--attached");
    expect(within(accountLibrary).getByText("Account One")).toBeInTheDocument();
    expect(within(accountLibrary).queryByText("Account Two")).not.toBeInTheDocument();
  });

  it("expands the account panel from the card's top-right toggle", async () => {
    const user = userEvent.setup();
    renderWorkspace(populatedWorkspaceProps());

    const front = document.querySelector(
      '[data-entitlement-group-card="premium"] .nt-entitlement-group-card__front',
    ) as HTMLElement;
    // Same slot the provider cards use, so both pages expand from the same spot.
    const toggle = front.querySelector(
      ".nt-entitlement-group-card__head-actions .nt-entitlement-group-card__library-toggle",
    ) as HTMLElement;

    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(
      within(front).queryByRole("button", { name: "Show Premium group accounts" }),
    ).toBe(toggle);
    expect(front.querySelectorAll(".nt-entitlement-group-card__action--icon")).toHaveLength(3);
    expect(within(front).queryByRole("button", { name: "Premium more actions" })).not.toBeInTheDocument();

    await user.click(toggle);

    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("region", { name: "Premium group accounts" })).toBeInTheDocument();
    expect(toggle).toHaveFocus();
  });

  it("opens a modal editor without expanding the card and restores focus on close", async () => {
    const user = userEvent.setup();
    const onSelectGroup = vi.fn();
    renderWorkspace(populatedWorkspaceProps({ onSelectGroup }));

    const card = screen.getByRole("article", { name: "Premium entitlement group card" });
    expect(screen.queryByRole("textbox", { name: "Group name" })).not.toBeInTheDocument();
    const edit = within(card).getByRole("button", { name: "Edit" });
    const toggle = within(card).getByRole("button", { name: "Premium" });
    await user.click(edit);

    expect(onSelectGroup).toHaveBeenCalledWith("row-premium");
    const dialog = screen.getByRole("dialog", { name: "Edit entitlement group" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(document.querySelector(".nt-group-admin__detail")).toBeNull();
    expect(within(dialog).getByRole("textbox", { name: "Group ID" })).toHaveValue("premium");
    expect(screen.getByRole("textbox", { name: "Group name" })).toHaveFocus();
    expect(within(dialog).getByRole("region", { name: "Member management" })).toBeInTheDocument();
    expect(document.body).toHaveAttribute("data-scroll-locked");

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => expect(edit).toHaveFocus());
    await user.click(edit);
    expect(screen.getByRole("textbox", { name: "Group name" })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(edit).toHaveFocus());
    expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  it("does not open the editor from a locked card", async () => {
    const user = userEvent.setup();
    const onSelectGroup = vi.fn();
    renderWorkspace(populatedWorkspaceProps({ editorLocked: true, onSelectGroup }));

    const card = screen.getByRole("article", { name: "Premium entitlement group card" });
    const edit = within(card).getByRole("button", { name: "Edit" });
    expect(edit).toBeDisabled();
    await user.click(edit);
    expect(onSelectGroup).not.toHaveBeenCalled();
    expect(screen.queryByRole("textbox", { name: "Group name" })).not.toBeInTheDocument();
  });

  it("reuses the credential pool account cards for resolved members", async () => {
    const user = userEvent.setup();
    renderWorkspace(populatedWorkspaceProps({ accountCards: accountCardBridge() }));

    const groupCard = document.querySelector('[data-entitlement-group-card="premium"]') as HTMLElement;
    await user.click(within(groupCard).getByRole("button", { name: "Premium" }));

    const accountLibrary = screen.getByRole("region", { name: "Premium group accounts" });
    expect(accountLibrary.querySelector(".nt-provider-account-card")).not.toBeNull();
    expect(accountLibrary.querySelector(".nt-entitlement-account-card")).toBeNull();
    expect(within(accountLibrary).getByText("Account One")).toBeInTheDocument();
  });

  it("falls back to the compact member cards when a member has no pool account", async () => {
    const user = userEvent.setup();
    renderWorkspace(
      populatedWorkspaceProps({ accountCards: accountCardBridge({ accountsById: new Map() }) }),
    );

    const groupCard = document.querySelector('[data-entitlement-group-card="premium"]') as HTMLElement;
    await user.click(within(groupCard).getByRole("button", { name: "Premium" }));

    const accountLibrary = screen.getByRole("region", { name: "Premium group accounts" });
    expect(accountLibrary.querySelector(".nt-entitlement-account-card")).not.toBeNull();
  });

  it("rolls member usage up onto the card front like the provider cards do", () => {
    renderWorkspace(populatedWorkspaceProps());

    const front = document.querySelector(
      '[data-entitlement-group-card="premium"] .nt-entitlement-group-card__front',
    ) as HTMLElement;
    const metric = (name: string) =>
      front.querySelector(`[data-entitlement-group-metric="${name}"]`)?.textContent;

    expect(metric("concurrency")).toBe("3/12");
    expect(metric("upstream-cost")).toBe("$4.50");
    expect(metric("platform-revenue")).toBe("$9.25");
    expect(metric("requests")).toBe("1,240");
    expect(metric("success-rate")).toBe("90%");
  });

  it("keeps the success rate control for groups that never dispatched", () => {
    renderWorkspace(
      populatedWorkspaceProps({
        groups: [{ ...populatedWorkspaceProps().groups[0], metrics: null }],
      }),
    );

    const front = document.querySelector(
      '[data-entitlement-group-card="premium"] .nt-entitlement-group-card__front',
    ) as HTMLElement;

    expect(front.querySelector('[data-entitlement-group-metric="concurrency"]')?.textContent).toBe(
      "—",
    );
    // The control stays put with a placeholder instead of vanishing, so cards do
    // not gain and lose a row depending on whether anyone dispatched.
    expect(front.querySelector('[data-entitlement-group-metric="success-rate"]')?.textContent).toBe(
      "—",
    );
    expect(within(front).getByText("No dispatch data yet")).toBeInTheDocument();
  });

  it("labels group configuration and usage without requiring icon tooltips", () => {
    renderWorkspace(populatedWorkspaceProps());

    const front = document.querySelector(
      '[data-entitlement-group-card="premium"] .nt-entitlement-group-card__front',
    ) as HTMLElement;

    expect(within(front).getByText("Providers")).toBeInTheDocument();
    expect(within(front).getByText("Accounts")).toBeInTheDocument();
    expect(within(front).getByText("Billing")).toBeInTheDocument();
    expect(front.querySelector('[data-entitlement-group-metric="accounts"]')?.textContent).toBe("1");
    expect(within(front).queryByText("Models")).not.toBeInTheDocument();
    expect(within(front).getByText("premium")).toBeInTheDocument();
    expect(within(front).getByRole("switch")).toHaveTextContent("Enabled");
    for (const label of ["Concurrency", "Upstream cost", "Revenue", "Requests", "Success rate"]) {
      expect(within(front).getByText(label)).toBeInTheDocument();
    }
    expect(within(front).queryByText("1 accounts")).not.toBeInTheDocument();
    expect(
      within(front).getByText("High-priority routing entitlements"),
    ).toBeInTheDocument();
    // Providers live on the back, in full, as selectable tabs.
    expect(within(front).queryByText("Managed OpenAI")).not.toBeInTheDocument();
  });

  it("narrows the expanded account panel to the providers picked on the back", async () => {
    const user = userEvent.setup();
    const anthropicPilotAccount: AccountsLedgerPilotAccount = {
      ...pilotAccount,
      accountId: "acct-2",
      providerId: "alt-anthropic",
      displayName: "Account Two",
      logicalLabels: ["Alt Anthropic"],
    };
    renderWorkspace(
      populatedWorkspaceProps({
        selectedGroupMembers: [
          ...populatedWorkspaceProps().selectedGroupMembers,
          {
            accountId: "acct-2",
            displayName: "Account Two",
            providerId: "alt-anthropic",
            providerLabel: "Alt Anthropic",
            vendorLabel: "Anthropic",
            mode: "credential",
            enabled: true,
            groupIds: ["premium"],
            groupLabels: ["Premium"],
            selected: true,
            searchText: "account two alt anthropic premium",
          },
        ],
        accountCards: accountCardBridge({
          accountsById: new Map([
            [pilotAccount.accountId, pilotAccount],
            [anthropicPilotAccount.accountId, anthropicPilotAccount],
          ]),
        }),
      }),
    );

    const groupCard = document.querySelector(
      '[data-entitlement-group-card="premium"]',
    ) as HTMLElement;
    await user.click(within(groupCard).getByRole("button", { name: "Premium" }));

    const accountLibrary = screen.getByRole("region", { name: "Premium group accounts" });
    expect(within(accountLibrary).getByText("Account One")).toBeInTheDocument();
    expect(within(accountLibrary).getByText("Account Two")).toBeInTheDocument();

    await user.click(within(groupCard).getByRole("button", { name: "Flip Premium for details" }));
    const back = groupCard.querySelector(".nt-entitlement-group-card__back") as HTMLElement;
    await user.click(
      back.querySelector('[data-entitlement-scope-provider="alt-anthropic"]') as HTMLElement,
    );

    // The card back and the panel below it read the same selection.
    expect(within(accountLibrary).getByText("Account One")).toBeInTheDocument();
    expect(within(accountLibrary).queryByText("Account Two")).not.toBeInTheDocument();
    expect(
      accountLibrary.querySelector(".nt-entitlement-group-card__accounts-count")?.textContent,
    ).toBe("1");
    expect(within(accountLibrary).getByText("Filtered to 1 providers")).toBeInTheDocument();
  });

  it("says so when the provider scope leaves the panel with nothing", async () => {
    const user = userEvent.setup();
    renderWorkspace(populatedWorkspaceProps({ accountCards: accountCardBridge() }));

    const groupCard = document.querySelector(
      '[data-entitlement-group-card="premium"]',
    ) as HTMLElement;
    await user.click(within(groupCard).getByRole("button", { name: "Premium" }));
    await user.click(within(groupCard).getByRole("button", { name: "Flip Premium for details" }));
    const back = groupCard.querySelector(".nt-entitlement-group-card__back") as HTMLElement;
    await user.click(within(back).getByRole("button", { name: "Clear" }));

    const accountLibrary = screen.getByRole("region", { name: "Premium group accounts" });
    expect(
      within(accountLibrary).getByText("No accounts fall inside the selected provider scope."),
    ).toBeInTheDocument();
  });

  it("aggregates every selected provider into one row per model on the back", async () => {
    const user = userEvent.setup();
    renderWorkspace(populatedWorkspaceProps());

    const groupCard = document.querySelector('[data-entitlement-group-card="premium"]') as HTMLElement;
    await user.click(within(groupCard).getByRole("button", { name: "Flip Premium for details" }));

    const back = groupCard.querySelector(".nt-entitlement-group-card__back") as HTMLElement;
    expect(within(back).getByText("Provider scope")).toBeInTheDocument();
    // Every provider starts selected, so the model band opens at full scope.
    const providerTab = (providerId: string) =>
      back.querySelector(`[data-entitlement-scope-provider="${providerId}"]`) as HTMLElement;
    expect(providerTab("managed-openai")).toHaveAttribute("aria-pressed", "true");
    expect(providerTab("alt-anthropic")).toHaveAttribute("aria-pressed", "true");

    const modelRow = (model: string) =>
      back.querySelector(`[data-entitlement-scope-model="${model}"]`) as HTMLElement | null;
    const modelMetric = (model: string, metric: string) =>
      modelRow(model)?.querySelector(`[data-entitlement-model-metric="${metric}"]`)?.textContent;

    expect(modelRow("claude-4.5")).not.toBeNull();
    // gpt-5 is served by both providers, so its row sums both member accounts.
    expect(modelMetric("gpt-5", "accounts")).toBe("1/2");
    expect(modelMetric("gpt-5", "concurrency")).toBe("4/16");
    expect(modelMetric("gpt-5", "upstream-cost")).toBe("$5.00");
    expect(modelMetric("gpt-5", "platform-revenue")).toBe("$11.00");
    expect(modelMetric("gpt-5", "requests")).toBe("1,300");
    expect(modelMetric("gpt-5", "success-rate")).toBe("75%");

    // Deselecting a provider narrows both the model list and every row's rollup.
    await user.click(providerTab("alt-anthropic"));

    expect(providerTab("alt-anthropic")).toHaveAttribute("aria-pressed", "false");
    expect(modelRow("claude-4.5")).toBeNull();
    expect(modelMetric("gpt-5", "accounts")).toBe("1/1");
    expect(modelMetric("gpt-5", "concurrency")).toBe("3/12");
    expect(modelMetric("gpt-5", "upstream-cost")).toBe("$4.50");
    expect(modelMetric("gpt-5", "platform-revenue")).toBe("$9.25");
    expect(modelMetric("gpt-5", "requests")).toBe("1,240");
    expect(modelMetric("gpt-5", "success-rate")).toBe("90%");
  });

  it("clears and restores the whole provider scope from one action", async () => {
    const user = userEvent.setup();
    renderWorkspace(populatedWorkspaceProps());

    const groupCard = document.querySelector('[data-entitlement-group-card="premium"]') as HTMLElement;
    await user.click(within(groupCard).getByRole("button", { name: "Flip Premium for details" }));
    const back = groupCard.querySelector(".nt-entitlement-group-card__back") as HTMLElement;

    await user.click(within(back).getByRole("button", { name: "Clear" }));

    expect(back.querySelector('[data-entitlement-scope-model="gpt-5"]')).toBeNull();
    expect(within(back).getByText("Select a provider tab to list models")).toBeInTheDocument();

    await user.click(within(back).getByRole("button", { name: "Select all" }));

    expect(back.querySelector('[data-entitlement-scope-model="gpt-5"]')).not.toBeNull();
  });
});
