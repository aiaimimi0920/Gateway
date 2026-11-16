import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { OperationsWorkspace } from "./OperationsWorkspace";
import { operationsWorkspaceFixture } from "./OperationsWorkspace.adoption-fixtures";
import { operationsRequestFixture } from "./OperationsWorkspace.fixtures";
import { OperationsRequestTable, requestOrigin } from "./OperationsRequestTable";
import { OperationsStatusStrip } from "./OperationsStatusStrip";
import { OperationsTrend } from "./OperationsTrend";

it("shows the summary preview and request filters without inventing missing metrics", () => {
  const props = operationsWorkspaceFixture({ onApplyRequestFilters: vi.fn() });
  render(<NeuroProvider><OperationsWorkspace {...props} /></NeuroProvider>);
  expect(screen.queryByRole("tablist")).not.toBeInTheDocument();
  expect(screen.getByRole("table", { name: "Requests" })).toBeVisible();
  expect(screen.queryByRole("button", { name: "Runtime details" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Fallback & error breakdown" })).not.toBeInTheDocument();
  for (const label of ["API key secret", "Fallback-eligible failures", "Fallback-exhausted failures", "Usage archive", "Prompt cache", "Generated at"]) {
    expect(screen.queryByText(label)).not.toBeInTheDocument();
  }
  expect(screen.getByText("Cache hit rate")).toBeVisible();
  expect(screen.getByText("Output speed")).toBeVisible();
  expect(screen.getByText("Billing unavailable")).toBeVisible();
  expect(screen.queryByText("Current concurrency")).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "Filter model" })).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "Filter provider" })).not.toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "Request ID" })).toBeVisible();
  expect(props.onApplyRequestFilters).not.toHaveBeenCalled();
});
it("uses audited provider identity, shows missing accounts honestly, and preserves error detail", async () => {
  const row = { ...operationsRequestFixture(), providerAccountId: "provider-a",
    routeTrace: { selectedCandidate: { providerLabel: "Recorded provider" }, realCredentialRef: "account-b", errorCode: "upstream_failed", attemptedProviderIds: ["provider-a", "provider-b"] },
    errorSummary: "Synthetic upstream timeout" };
  const user = userEvent.setup();
  render(<NeuroProvider><OperationsRequestTable rows={[row]} t={(_, en) => en} /></NeuroProvider>);
  expect(screen.getByText("Recorded provider")).toBeVisible();
  expect(screen.getByText("Account: account-b")).toBeVisible();
  await user.click(screen.getByRole("button", { name: `Request details: ${row.id}` }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.queryByText("AGENT")).not.toBeInTheDocument();
  const dialog = within(screen.getByRole("region", { name: "Request details" }));
  expect(dialog.getByText("upstream_failed")).toBeVisible();
  expect(dialog.getByText("Synthetic upstream timeout")).toBeVisible();
  expect(dialog.getByText("provider-a → provider-b")).toBeVisible();
  await user.click(screen.getByRole("button", { name: `Request details: ${row.id}` }));
  expect(screen.queryByRole("region", { name: "Request details" })).not.toBeInTheDocument();
  expect(requestOrigin({ ...row, routeTrace: null })).toMatchObject({ provider: "provider-a", account: null, attempts: [] });
  expect(requestOrigin({ ...row, providerAccountId: null, routeTrace: { selectedCandidate: "invalid", attemptedProviderIds: [false, "valid"] } })).toMatchObject({ provider: null, account: null, attempts: ["valid"] });
});

it("summarizes only the supplied audits and keeps unknown token usage unknown", () => {
  const row = { ...operationsRequestFixture(), totalTokens: 120, promptTokens: 100, completionTokens: 20 };
  const view = render(<OperationsStatusStrip rows={[row]} t={(_, en) => en} />);
  expect(screen.getByText("120")).toBeVisible();
  expect(screen.getByText("Input 100 · Output 20")).toBeVisible();
  view.rerender(<OperationsStatusStrip rows={[{ ...row, totalTokens: null }]} t={(_, en) => en} />);
  expect(screen.queryByText("120")).not.toBeInTheDocument();
  expect(screen.getByText("Token").nextElementSibling).toHaveTextContent("—");
});

it("switches between grouped bars and chronological lines without inventing missing usage", async () => {
  const user = userEvent.setup();
  const row = { ...operationsRequestFixture(), createdAt: "2026-11-15T10:00:00Z", totalTokens: 120 };
  render(<NeuroProvider><OperationsTrend rows={[row]} t={(_, en) => en} /></NeuroProvider>);
  expect(screen.getByRole("button", { name: "Bar chart" })).toHaveAttribute("aria-pressed", "true");
  await user.click(screen.getByRole("button", { name: "Line chart" }));
  expect(screen.getByRole("img", { name: "Per-minute request trend" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Line chart" })).toHaveAttribute("aria-pressed", "true");
  await user.click(screen.getByRole("radio", { name: "Provider" }));
  expect(screen.getByRole("radio", { name: "Provider" })).toBeChecked();
  expect(screen.getByRole("img", { name: "Per-minute request trend" })).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Bar chart" }));
  expect(screen.getByRole("img", { name: "Per-minute grouped bars" })).toBeVisible();
  expect(screen.getByRole("radio", { name: "Provider" })).toBeChecked();
  expect(screen.getAllByText("120").length).toBeGreaterThan(0);
});
