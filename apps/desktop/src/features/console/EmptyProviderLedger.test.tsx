import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { AccountsLedgerWorkspace } from "./AccountsLedgerWorkspace";
import { workspaceProps } from "./AccountsLedgerWorkspace.fixtures";
import { buildAccountLedgerSections } from "./accountLedgerSections";
import { buildAccountLedgerRows, buildRouteAccountCatalog } from "./accountManagementViewModel";
import { buildConsoleTelemetrySnapshot } from "./telemetry";
import { buildRouteAccountCatalog as buildSharedCatalog } from "./routeAccountCatalog";

const snapshot = buildConsoleTelemetrySnapshot({
  pressure: null, costOverview: null,
  credentialModelStates: [], credentialInventory: null, requestAuditSummary: null
});
const empty = {
  id: "nvidia", preset: "nvidia-openai", base_url: "https://integrate.api.nvidia.com",
  explicit_credentials_only: true, credentials: []
};

it("shows the newly created empty provider and lets its account library add the first credential", async () => {
  const document = { providers: [empty], model_routes: [], aliases: {} };
  const rows = buildAccountLedgerRows(buildRouteAccountCatalog(document), {});
  expect(rows).toEqual([]);
  const sections = buildAccountLedgerSections(document, rows, new Map(), snapshot, "zh-CN");
  expect(sections).toHaveLength(1);
  expect(sections[0]).toMatchObject({ providerId: "nvidia", directAccounts: [], hasExplicitAccounts: false });
  const onAddExplicit = vi.fn();
  render(<NeuroProvider><AccountsLedgerWorkspace {...workspaceProps({
    pilotSections: sections, totalAccounts: 0,
    rows: [], onAddExplicit
  })} /></NeuroProvider>);
  expect(screen.queryByRole("heading", { name: "暂无账号" })).not.toBeInTheDocument();
  const user = userEvent.setup();
  await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /^nvidia/ }));
  expect(screen.getByRole("switch", { name: "服务商调度" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "添加账号" }));
  expect(onAddExplicit).toHaveBeenCalledExactlyOnceWith("nvidia");
});

it("does not resurrect a nonempty provider whose account rows were filtered out", () => {
  const document = {
    providers: [{ ...empty, credentials: [{ id: "one", api_key: "fixture" }] }],
    model_routes: [], aliases: {}
  };
  expect(buildAccountLedgerSections(document, [], new Map(), snapshot, "zh-CN")).toEqual([]);
});

it("both catalog entrypoints preserve legacy defaults but never fabricate empty-pool accounts", () => {
  const legacy = { ...empty, id: "legacy", explicit_credentials_only: false };
  const document = { providers: [empty, legacy], model_routes: [], aliases: {} };
  for (const build of [buildRouteAccountCatalog, buildSharedCatalog]) {
    expect(build(document).accounts.map(({ id }) => id)).toEqual(["legacy::default"]);
    expect(build(document).providerBuckets.flatMap(({ providers }) => providers)
      .find(({ id }) => id === "nvidia")?.accounts).toEqual([]);
  }
});
