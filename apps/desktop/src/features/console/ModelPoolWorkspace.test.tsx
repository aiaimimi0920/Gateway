import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

import { ModelPoolWorkspace, type ModelPoolWorkspaceProps } from "./ModelPoolWorkspace";
import type { ModelPoolCard } from "./modelPoolViewModel";
import { NeuroProvider } from "@neuro/ui";

afterEach(cleanup);

it("bounds directory rows and searches the full catalog with page reset", async () => {
  const models = Array.from({ length: 441 }, (_, index) => modelCard(`model-${index}`));
  const { container, props, rerender, user } = renderWorkspace({ models });
  const cards = () => container.querySelectorAll(".nt-directory-model-name");
  const pager = () => within(screen.getByRole("navigation", { name: "Model list pagination" }));
  expect(cards()).toHaveLength(24);
  expect(pager().getByText("1 / 19")).toBeVisible();
  await user.click(pager().getByRole("button", { name: "Next page" }));
  expect(cards()[0]).toHaveTextContent(/^model-24$/);
  rerender(<ModelPoolWorkspace {...props} poolView={{ query: "model-440", sort: "default" }} />);
  expect(cards()).toHaveLength(1);
  expect(cards()[0]).toHaveTextContent(/^model-440$/);
  rerender(<ModelPoolWorkspace {...props} poolView={{ query: "", sort: "default" }} />);
  expect(pager().getByText("1 / 19")).toBeVisible();
  await user.click(pager().getByRole("button", { name: "Next page" }));
  rerender(<ModelPoolWorkspace {...props} poolView={{ query: "", sort: "name-desc" }} />);
  expect(pager().getByText("1 / 19")).toBeVisible();
  rerender(<ModelPoolWorkspace {...props} models={models.slice(0, 1)} />);
  expect(cards()).toHaveLength(1);
});

it("mounts the editable provider chain only while its model is expanded", async () => {
  const { user, props } = renderWorkspace();
  const name = `${props.models[0].model} provider priority`;
  const toggle = screen.getByRole("button", { name: props.models[0].model });
  expect(screen.queryByRole("region", { name })).not.toBeInTheDocument();
  await user.click(toggle);
  const chain = screen.getByRole("region", { name });
  expect(within(chain).getByText("Second provider")).toBeVisible();
  expect(toggle).toHaveAttribute("aria-expanded", "true");
  expect(toggle).toHaveAttribute("aria-controls", chain.id);
  await user.click(toggle);
  expect(screen.queryByRole("region", { name })).not.toBeInTheDocument();
  expect(toggle).toHaveAttribute("aria-expanded", "false");
});

function modelCard(model = "gemini-2.5-pro:free"): ModelPoolCard {
  return {
    rowId: model, model, accountCount: 3, enabledAccountCount: 3,
    pinned: true, enabled: true, routedPatterns: [model], metrics: null,
    chain: [
      {
        providerId: "first", providerLabel: "First provider", accountCount: 1,
        enabledAccountCount: 1, accountIds: ["account-a"], detached: false, metrics: null,
      },
      {
        providerId: "second", providerLabel: "Second provider", accountCount: 2,
        enabledAccountCount: 2, accountIds: ["account-b", "account-c"],
        detached: false, metrics: null,
      },
    ],
  };
}

function renderWorkspace(overrides: Partial<ModelPoolWorkspaceProps> = {}) {
  const props: ModelPoolWorkspaceProps = {
    t: (_zh, en) => en, models: [modelCard()], editorLocked: false,
    onMoveProvider: vi.fn(), onResetChain: vi.fn(), onToggleEnabled: vi.fn(),
    onEditModel: vi.fn(), onDeleteModel: vi.fn(), ...overrides,
  };
  return { ...render(<ModelPoolWorkspace {...props} />, { wrapper: NeuroProvider }), props, user: userEvent.setup() };
}

it("renders directory identities and counts and distinguishes empty search from an empty catalog", () => {
  const { props, rerender } = renderWorkspace({ notice: "Notice" });
  expect(screen.getByText("Notice")).toBeVisible();
  const row = within(screen.getByRole("table", { name: "Model directory" })).getAllByRole("row")[1];
  expect(within(row).getByRole("button", { name: props.models[0].model })).toBeVisible();
  expect(within(row).getByText("2 sources")).toBeVisible();
  expect(within(row).getByRole("cell", { name: "3" })).toBeVisible();
  expect(within(row).getByTitle("First provider · Second provider")).toBeVisible();
  rerender(<ModelPoolWorkspace {...props} poolView={{ query: "missing-model", sort: "default" }} />);
  expect(screen.getByText("No matching models. Change or clear the search.")).toBeVisible();
  expect(screen.queryByRole("switch")).not.toBeInTheDocument();
  rerender(<ModelPoolWorkspace {...props} models={[]} />);
  expect(screen.getByText("No models yet").closest(".n-empty")).toBeVisible();
  expect(within(screen.getByRole("table", { name: "Model directory" })).getAllByRole("row")).toHaveLength(1);
});

it("preserves keyboard focus while expanding and collapsing model details", async () => {
  const { user, props } = renderWorkspace();
  expect(screen.queryByRole("button", { name: /more actions/ })).not.toBeInTheDocument();
  const toggle = screen.getByRole("button", { name: props.models[0].model });
  toggle.focus();
  await user.keyboard("{Enter}");
  expect(toggle).toHaveFocus();
  expect(toggle).toHaveAttribute("aria-expanded", "true");
  expect(screen.getByRole("button", { name: "Edit model" })).toBeVisible();
  await user.keyboard(" ");
  expect(toggle).toHaveFocus();
  expect(toggle).toHaveAttribute("aria-expanded", "false");
  expect(screen.queryByRole("button", { name: "Edit model" })).not.toBeInTheDocument();
});

it("dispatches exact model actions and resets priority from the expanded directory row", async () => {
  const { user, props } = renderWorkspace();
  const model = props.models[0].model;
  await user.click(screen.getByRole("switch"));
  await user.click(screen.getByRole("button", { name: model }));
  await user.click(screen.getByRole("button", { name: "Edit model" }));
  await user.click(screen.getByRole("button", { name: "Delete model" }));
  expect(props.onToggleEnabled).toHaveBeenCalledExactlyOnceWith(model, false);
  expect(props.onEditModel).toHaveBeenCalledExactlyOnceWith(model);
  expect(props.onDeleteModel).toHaveBeenCalledExactlyOnceWith(model);
  await user.click(screen.getByRole("button", { name: "Restore inherited order" }));
  expect(props.onResetChain).toHaveBeenCalledExactlyOnceWith(model);
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
});

it("honors editor locks and the inherited-chain reset guard", async () => {
  const { user, props, rerender } = renderWorkspace({ editorLocked: true });
  expect(screen.getByRole("switch")).toBeDisabled();
  await user.click(screen.getByRole("button", { name: props.models[0].model }));
  expect(screen.getByRole("button", { name: "Edit model" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Delete model" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Restore inherited order" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Move First provider down" })).toBeDisabled();
  rerender(<ModelPoolWorkspace {...props} editorLocked={false} models={[{ ...modelCard(), pinned: false }]} />);
  expect(screen.getByRole("button", { name: "Restore inherited order" })).toBeDisabled();
  expect(props.onResetChain).not.toHaveBeenCalled();
});

it("shares provider selection with the attached account panel and forwards chain moves", async () => {
  const { user, container, props } = renderWorkspace();
  const model = props.models[0].model;
  await user.click(screen.getByRole("button", { name: model }));
  await user.click(screen.getByRole("button", { name: "View all accounts" }));
  const panel = screen.getByRole("region", { name: `${model} serving accounts` });
  expect(panel).toHaveAttribute("id", `model-sources-${encodeURIComponent(model)}-accounts`);
  expect(panel).toHaveClass("nt-provider-account-library", "nt-provider-account-library--attached");
  expect(within(panel).getByText(`${model} account library`)).toBeVisible();
  const count = () => panel.querySelector(".nt-provider-account-library__count");
  expect(count()).toHaveTextContent("3");
  await user.click(screen.getByRole("button", { name: "Move First provider down" }));
  expect(props.onMoveProvider).toHaveBeenCalledExactlyOnceWith(model, "first", "down");
  const priority = within(screen.getByRole("region", { name: `${model} provider priority` }));
  const providerRow = priority.getByRole("row", { name: /Second provider/ });
  await user.click(within(providerRow).getByRole("button", { name: "2" }));
  expect(within(panel).getByText("Filtered to 1 providers")).toBeVisible();
  expect(count()).toHaveTextContent("2");
  await user.click(within(priority.getByRole("row", { name: /First provider/ })).getByRole("button", { name: "1" }));
  expect(count()).toHaveTextContent("1");
  await user.click(screen.getByRole("button", { name: "Hide accounts" }));
  expect(screen.queryByRole("region", { name: `${model} serving accounts` })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "View all accounts" }));
  const allPanel = screen.getByRole("region", { name: `${model} serving accounts` });
  expect(allPanel.querySelector(".nt-provider-account-library__count")).toHaveTextContent("3");
  expect(container.querySelectorAll(".nt-provider-account-library--attached")).toHaveLength(1);
  expect(allPanel.querySelector(".nt-provider-account-library__pager")).toBeInTheDocument();
});

it("expands one framed model library at a time without an overflow menu", async () => {
  const { user } = renderWorkspace({ models: [modelCard("alpha"), modelCard("beta")] });
  await user.click(screen.getByRole("button", { name: "alpha" }));
  await user.click(screen.getByRole("button", { name: "View all accounts" }));
  expect(screen.getByRole("region", { name: "alpha serving accounts" })).toBeVisible();
  await user.click(screen.getByRole("button", { name: "beta" }));
  await user.click(screen.getByRole("button", { name: "View all accounts" }));
  expect(screen.queryByRole("region", { name: "alpha serving accounts" })).not.toBeInTheDocument();
  expect(screen.getByRole("region", { name: "beta serving accounts" })).toBeVisible();
  await user.click(screen.getByRole("button", { name: "beta" }));
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  expect(screen.queryByRole("region", { name: "beta serving accounts" })).not.toBeInTheDocument();
});
