import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

import { ModelPoolWorkspace, type ModelPoolWorkspaceProps } from "./ModelPoolWorkspace";
import type { ModelPoolCard } from "./modelPoolViewModel";

afterEach(cleanup);

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
  return { ...render(<ModelPoolWorkspace {...props} />), props, user: userEvent.setup() };
}

it("renders aggregate metrics and preserves the missing-data and empty states", () => {
  const card = modelCard();
  card.metrics = {
    concurrency: { used: 2, total: 5 }, upstreamCost: 1.25, platformRevenue: 2.5,
    requests: 1234, successWindows: [{ label: "10:00", success: 3, requests: 4, rate: 0.75 }],
    successSuccessCount: 3, successRequestCount: 4, successRate: 0.75,
  };
  const { container, props, rerender } = renderWorkspace({ models: [card], notice: "Notice" });
  const metric = (name: string) => container.querySelector(`[data-model-pool-metric="${name}"]`);
  expect(screen.getByText("Notice")).toBeVisible();
  expect(metric("providers")).toHaveTextContent("2");
  expect(metric("accounts")).toHaveTextContent("3");
  expect(metric("primary")).toHaveTextContent("First provider");
  expect(metric("concurrency")).toHaveTextContent("2/5");
  expect(metric("upstream-cost")).toHaveTextContent("$1.25");
  expect(metric("platform-revenue")).toHaveTextContent("$2.50");
  expect(metric("requests")).toHaveTextContent("1,234");
  expect(screen.getByRole("img")).toHaveAccessibleName(/3\/4 successful/);
  expect(container.querySelectorAll(".nt-provider-card__availability-cell").length).toBeGreaterThan(0);
  rerender(<ModelPoolWorkspace {...props} models={[modelCard()]} />);
  expect(within(screen.getByRole("img")).getByText("No dispatch data yet")).toBeVisible();
  expect(screen.getByRole("img")).toHaveAccessibleName(/unavailable/);
  rerender(<ModelPoolWorkspace {...props} models={[]} />);
  expect(screen.getByRole("heading", { name: "No models yet" })).toBeVisible();
  expect(screen.queryByRole("region", { name: "Model cards" })).not.toBeInTheDocument();
});

it("removes the model overflow menu and preserves flip focus and inaccessible inactive faces", async () => {
  const { user, container } = renderWorkspace();
  expect(screen.queryByRole("button", { name: /more actions/ })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: /to reorder its chain/ }));
  const backFlip = screen.getByRole("button", { name: /back to the front/ });
  expect(backFlip).toHaveFocus();
  expect(container.querySelector(".nt-entitlement-group-card__front")).toHaveAttribute("inert");
  expect(screen.queryByRole("switch")).not.toBeInTheDocument();
  expect(screen.getByRole("status")).toHaveTextContent("card flipped to the back");
  await user.keyboard(" ");
  expect(screen.getByRole("button", { name: /to reorder its chain/ })).toHaveFocus();
  expect(container.querySelector(".nt-entitlement-group-card__back")).toHaveAttribute("inert");
  expect(screen.getByRole("status")).toHaveTextContent("card flipped to the front");
});

it("dispatches exact model actions and resets priority directly from the back", async () => {
  const { user, props } = renderWorkspace();
  const model = props.models[0].model;
  await user.click(screen.getByRole("switch"));
  await user.click(screen.getByRole("button", { name: "Edit" }));
  await user.click(screen.getByRole("button", { name: "Delete" }));
  expect(props.onToggleEnabled).toHaveBeenCalledExactlyOnceWith(model, false);
  expect(props.onEditModel).toHaveBeenCalledExactlyOnceWith(model);
  expect(props.onDeleteModel).toHaveBeenCalledExactlyOnceWith(model);
  await user.click(screen.getByRole("button", { name: /to reorder its chain/ }));
  await user.click(screen.getByRole("button", { name: "Reset chain" }));
  expect(props.onResetChain).toHaveBeenCalledExactlyOnceWith(model);
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
});

it("honors editor locks and the inherited-chain reset guard", async () => {
  const { user, props, rerender } = renderWorkspace({ editorLocked: true });
  expect(screen.getByRole("switch")).toBeDisabled();
  expect(screen.getByRole("button", { name: "Edit" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Delete" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: /to reorder its chain/ }));
  expect(screen.getByRole("button", { name: "Reset chain" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Move First provider down one slot" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: /back to the front/ }));
  rerender(<ModelPoolWorkspace {...props} editorLocked={false} models={[{ ...modelCard(), pinned: false }]} />);
  await user.click(screen.getByRole("button", { name: /to reorder its chain/ }));
  expect(screen.getByRole("button", { name: "Reset chain" })).toBeDisabled();
  expect(props.onResetChain).not.toHaveBeenCalled();
});

it("shares provider selection with the attached account panel and forwards chain moves", async () => {
  const { user, container, props } = renderWorkspace();
  const model = props.models[0].model;
  await user.click(screen.getByRole("button", { name: `Show ${model} serving accounts` }));
  const panel = screen.getByRole("region", { name: `${model} serving accounts` });
  expect(panel).toHaveAttribute("id", "model-pool-accounts-gemini-2-5-pro-free");
  expect(panel).toHaveClass("nt-provider-account-library", "nt-provider-account-library--attached");
  expect(within(panel).getByText(`${model} account library`)).toBeVisible();
  const count = () => panel.querySelector(".nt-provider-account-library__count");
  expect(count()).toHaveTextContent("3");
  await user.click(screen.getByRole("button", { name: /to reorder its chain/ }));
  await user.click(screen.getByRole("button", { name: "Move First provider down one slot" }));
  expect(props.onMoveProvider).toHaveBeenCalledExactlyOnceWith(model, "first", "down");
  await user.click(screen.getByRole("button", { name: "First provider" }));
  expect(within(panel).getByText("Filtered to 1 providers")).toBeVisible();
  expect(count()).toHaveTextContent("2");
  await user.click(screen.getByRole("button", { name: "Second provider" }));
  expect(count()).toHaveTextContent("0");
  expect(within(panel).getByText("No accounts fall inside the selected provider scope.")).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Select all" }));
  expect(count()).toHaveTextContent("3");
  expect(container.querySelectorAll(".nt-provider-account-library--attached")).toHaveLength(1);
  expect(panel.querySelector(".nt-provider-account-library__pager")).toBeInTheDocument();
});

it("expands one framed model library at a time without an overflow menu", async () => {
  const { user } = renderWorkspace({ models: [modelCard("alpha"), modelCard("beta")] });
  await user.click(screen.getByRole("button", { name: "Show alpha serving accounts" }));
  await user.click(screen.getByRole("button", { name: "Show beta serving accounts" }));
  expect(screen.queryByRole("region", { name: "alpha serving accounts" })).not.toBeInTheDocument();
  expect(screen.getByRole("region", { name: "beta serving accounts" })).toBeVisible();
  await user.click(screen.getByRole("button", { name: "beta" }));
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  expect(screen.queryByRole("region", { name: "beta serving accounts" })).not.toBeInTheDocument();
});
