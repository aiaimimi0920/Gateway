import { NeuroProvider } from "@neuro/ui";
import { fireEvent, screen, within } from "@testing-library/react";
import { renderNeuro as render } from "../../test/renderNeuro";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { OperationsWorkspace } from "./OperationsWorkspace";
import { OperationsToolbar } from "./OperationsToolbar";
import { operationsWorkspaceFixture } from "./OperationsWorkspace.adoption-fixtures";

it("keeps the request ledger primary without restoring removed diagnostic disclosures", () => {
  const props = operationsWorkspaceFixture();
  const { rerender, container } = render(<OperationsWorkspace {...props} />);
  const table = screen.getByRole("table", { name: "Requests" });
  expect(table).toBeVisible();
  expect(screen.queryByText("Account & project load")).not.toBeInTheDocument();
  const pressure = { ...props.pressure, data: { ...props.pressure.data!, providers: [{ ...props.pressure.data!.providers[0], breakerOpen: true }] } };
  rerender(<OperationsWorkspace {...props} pressure={pressure} />);
  expect(screen.getByRole("table", { name: "Requests" })).toBe(table);
  rerender(<OperationsWorkspace {...props} pressure={{ ...pressure, loading: true }} />);
  expect(screen.queryByText("Dependency & configuration checks")).not.toBeInTheDocument();
  expect(screen.queryByRole("table", { name: "Project load" })).not.toBeInTheDocument();
  expect(screen.getByRole("table", { name: "Requests" })).toBe(table);
  expect(container.querySelector(".nt-table, .nt-badge, .nt-alert")).toBeNull();
});

it("preserves request rows and keyboard focus when only sort, read state or metrics change", async () => {
  const props = operationsWorkspaceFixture();
  const { rerender } = render(<OperationsWorkspace {...props} />);
  const table = screen.getByRole("table", { name: "Requests" });
  const toggle = within(table).getByRole("button", { name: "Request details: request-a" });
  const row = toggle.closest("tr")!;
  fireEvent.click(toggle);
  const filter = screen.getByRole("textbox", { name: "Request ID" });
  filter.focus();
  rerender(<OperationsWorkspace {...props} listView={{ query: "", sort: "name-desc" }} requests={{ ...props.requests, error: "stale read", loading: true }} />);
  expect(filter).toHaveFocus();
  expect(within(table).getByRole("button", { name: "Request details: request-a" }).closest("tr")).toBe(row);
  expect(toggle).toHaveAttribute("aria-expanded", "true");
  expect(screen.getByText("stale read")).toBeVisible();
  expect(within(screen.getByRole("region", { name: "Request details" })).getByText("upstream rate limited")).toBeVisible();
});

it("keeps three independent server views, stable incident targets, automation and truncation semantics", async () => {
  const user = userEvent.setup();
  const props = operationsWorkspaceFixture({ onResolveIncident: vi.fn() });
  const { rerender } = render(<OperationsWorkspace {...props} />);
  await user.click(screen.getByRole("button", { name: "Incident records" }));
  await user.click(screen.getByText("Anomaly policies"));
  expect(screen.getByRole("table", { name: "Queued alerts" })).toHaveTextContent("Due");
  expect(screen.getByRole("table", { name: "Anomaly policies" })).toHaveTextContent("SyncAlertsEscalateRemediate");
  const row = screen.getByRole("rowheader", { name: "incident-b" }).closest("tr")!;
  const resolve = within(row).getByRole("button", { name: "Resolve" });
  resolve.focus();
  rerender(<OperationsWorkspace {...props} incidentBusyId="incident-a" />);
  expect(resolve).toHaveFocus();
  expect(resolve).toBeEnabled();
  await user.click(resolve);
  expect(props.onResolveIncident).toHaveBeenCalledExactlyOnceWith("incident-b");
  await user.click(screen.getByRole("tab", { name: "Automated remediation" }));
  expect(screen.queryByRole("table", { name: "Incidents" })).not.toBeInTheDocument();
  expect(screen.getByRole("table", { name: "Queued remediation" })).toHaveTextContent("Waiting for approval");
  await user.click(screen.getByText("Remediation runs"));
  const runs = screen.getByRole("table", { name: "Remediation runs" });
  expect(runs).toHaveTextContent("Dry run");
  expect(within(runs).getByTitle("incident-very-long-stable-identifier")).toHaveTextContent("incident-ver…");
  await user.click(screen.getByRole("tab", { name: "Analysis & exports" }));
  await user.click(screen.getByText("Exports", { selector: "summary > span > span" }));
  const exports = screen.getByRole("table", { name: "Exports" });
  expect(within(exports).getAllByRole("rowheader")).toHaveLength(20);
  expect(within(exports).getByRole("rowheader", { name: "export-0" })).toHaveAttribute("scope", "row");
  expect(within(exports).queryByText("Export 20")).not.toBeInTheDocument();
  expect(within(exports).getAllByTitle("2026-11-10T01:00:00Z")).toHaveLength(20);
  expect(screen.queryByRole("table", { name: "Remediation runs" })).not.toBeInTheDocument();
});

it("reserves the search clear control through empty, edited and disabled states", () => {
  const onChange = vi.fn();
  const { rerender } = render(<NeuroProvider><OperationsToolbar t={(_, en) => en} value={{ query: "", sort: "default" }} onChange={onChange} /></NeuroProvider>);
  const clear = screen.getByRole("button", { name: "Clear operations search" });
  expect(clear).toBeDisabled();
  rerender(<NeuroProvider><OperationsToolbar t={(_, en) => en} value={{ query: "request-a", sort: "default" }} onChange={onChange} /></NeuroProvider>);
  expect(screen.getByRole("button", { name: "Clear operations search" })).toBe(clear);
  fireEvent.click(clear);
  expect(onChange).toHaveBeenCalledExactlyOnceWith({ query: "", sort: "default" });
});

it("keeps asynchronous empty-result announcements when standard empty states replace legacy strips", () => {
  const props = operationsWorkspaceFixture({ requests: { data: [], loading: false, error: null } });
  const { rerender } = render(<OperationsWorkspace {...props} />);
  const empty = screen.getByText("No matching requests");
  expect(empty.closest('[role="status"]')).not.toBeNull();
  rerender(<OperationsWorkspace {...props} requestSummary={{ data: null, error: null, loading: false }} />);
  expect(screen.getByText("No matching requests")).toBe(empty);
});
