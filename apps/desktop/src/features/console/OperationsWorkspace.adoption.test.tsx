import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { OperationsWorkspace } from "./OperationsWorkspace";
import { OperationsToolbar } from "./OperationsToolbar";
import { operationsWorkspaceFixture } from "./OperationsWorkspace.adoption-fixtures";

it("uses standard semantic tables with row headings and retains diagnostics across observations", async () => {
  const user = userEvent.setup();
  const props = operationsWorkspaceFixture();
  const { rerender, container } = render(<OperationsWorkspace {...props} />);
  const load = screen.getByText("Account & project load");
  await user.click(load);
  const row = screen.getByRole("rowheader", { name: "Account A" });
  expect(screen.getByRole("table", { name: "Project load" })).toBeVisible();
  await user.click(load);
  const pressure = { ...props.pressure, data: { ...props.pressure.data!, providers: [{ ...props.pressure.data!.providers[0], breakerOpen: true }] } };
  rerender(<OperationsWorkspace {...props} pressure={pressure} />);
  expect(row).toBeVisible();
  expect(screen.getByRole("rowheader", { name: "Account A" })).toBe(row);
  await user.click(load);
  rerender(<OperationsWorkspace {...props} pressure={{ ...pressure, loading: true }} />);
  expect(load.closest("details")).not.toHaveAttribute("open");
  expect(screen.getByRole("rowheader", { name: "Account A", hidden: true })).toBe(row);
  await user.click(screen.getByText("Dependency & configuration checks"));
  expect(screen.getByRole("table", { name: "Dependency state" })).toBeVisible();
  expect(screen.getByRole("rowheader", { name: "PostgreSQL" })).toBeVisible();
  expect(screen.queryByRole("rowheader", { name: "Redis" })).not.toBeInTheDocument();
  expect(container.querySelector(".nt-table, .nt-badge, .nt-alert")).toBeNull();
});

it("preserves request rows and keyboard focus when only sort, read state or metrics change", async () => {
  const props = operationsWorkspaceFixture();
  const { rerender } = render(<OperationsWorkspace {...props} />);
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Request log" }));
  const table = screen.getByRole("table", { name: "Requests" });
  const row = within(table).getAllByRole("rowheader")[0].closest("tr")!;
  const filter = screen.getByRole("combobox", { name: "Status" });
  filter.focus();
  rerender(<OperationsWorkspace {...props} listView={{ query: "", sort: "name-desc" }} requests={{ ...props.requests, error: "stale read", loading: true }} />);
  expect(filter).toHaveFocus();
  expect(within(table).getAllByRole("rowheader")[1].closest("tr")).toBe(row);
  expect(row).toHaveTextContent("Stream");
  expect(row.lastElementChild).toHaveTextContent("—");
  expect(screen.getByText("stale read")).toBeVisible();
  expect(within(row).getByText("upstream rate limited")).toHaveAttribute("title", "upstream rate limited");
});

it("keeps three independent server views, stable incident targets, automation and truncation semantics", async () => {
  const user = userEvent.setup();
  const props = operationsWorkspaceFixture({ onResolveIncident: vi.fn() });
  const { rerender } = render(<OperationsWorkspace {...props} />);
  await user.click(screen.getByRole("tab", { name: /Incident management/ }));
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
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Request log" }));
  expect(screen.getByText("No matching requests").closest('[role="status"]')).not.toBeNull();
  rerender(<OperationsWorkspace {...props} requestSummary={{ data: null, error: null, loading: false }} />);
  expect(screen.getByText("No data yet").closest('[role="status"]')).not.toBeNull();
});
