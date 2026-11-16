import { renderHook } from "@testing-library/react";
import { expect, it } from "vitest";
import { buildAccountLedgerRows } from "./accountManagementViewModel";
import { buildAccountLedgerSections } from "./accountLedgerSections";
import { buildRouteAccountCatalog } from "./routeDocumentAccountCatalog";
import { buildRouteAccountCatalog as buildConsoleCatalog } from "./routeAccountCatalog";
import { parseRouteDocument } from "./routeDocument";
import { providerAccessKind, providerAccessLabel } from "./providerAccessKind";
import { providerPoolEntries, providerPoolTableRows } from "./providerPoolView";
import { useConsoleCredentialSelectors } from "./useConsoleCredentialSelectors";
import { buildConsoleTelemetrySnapshot } from "./telemetry";
import { useConsoleAccountSelectors } from "./useConsoleAccountSelectors";

const presets = ["google-agent-platform", "google-agent-platform-official-api", "vertex-gemini", "vertex-official-api"];
function document() {
  return parseRouteDocument(JSON.stringify({ providers: [
    { id: "renamed-full", label: "Full", preset: presets[1], auth_mode: "vertex_user_oauth", credentials: [{ id: "stable-full" }] },
    { id: "express", label: "Express", preset: "vertex-express-api", auth_mode: "vertex_express_api_key", credentials: [{ id: "express-key" }] },
    { id: "vertex-oauth", label: "Legacy", preset: presets[1], credentials: [{ id: "legacy" }] },
  ], model_routes: [], aliases: {} }));
}

it.each(presets)("classifies typed Full Vertex alias %s without guessing from ID or label", (providerPreset) => {
  const metadata = { providerId: "renamed", providerLabel: "My pool-api", providerPreset, authMode: "vertex_user_oauth" };
  expect(providerAccessKind(metadata)).toBe("oauth");
  expect(providerAccessLabel(metadata)).toBe("My pool-oauth");
  expect(providerAccessKind({ ...metadata, authMode: null })).toBe("api");
  expect(providerAccessKind({ ...metadata, providerPreset: "openai" })).toBe("api");
});

it.each([["console", buildConsoleCatalog], ["extracted", buildRouteAccountCatalog]] as const)(
  "keeps %s card and table filters on typed identity and leaves source data unchanged", (_, buildCatalog) => {
  const source = document();
  const before = JSON.stringify(source);
  const rows = buildAccountLedgerRows(buildCatalog(source), {});
  const snapshot = buildConsoleTelemetrySnapshot({ pressure: null, costOverview: null,
    credentialModelStates: [], credentialInventory: null, requestAuditSummary: null });
  const sections = buildAccountLedgerSections(source, rows, new Map(), snapshot, "zh-CN");
  const view = { query: "", sort: "default" as const, accessKinds: ["oauth" as const] };
  expect(providerPoolEntries(sections, view).map(({ section }) => section.providerId)).toEqual(["renamed-full"]);
  expect(providerPoolTableRows(rows, view).map((row) => row.accountId)).toEqual(["stable-full"]);
  expect(providerPoolEntries(sections, { ...view, query: "full-oauth" })).toHaveLength(1);
  expect(providerPoolTableRows(rows, { ...view, query: "full-oauth" })).toHaveLength(1);
  expect(JSON.stringify(source)).toBe(before);
});

it("uses the same typed identity in normal account provider options", () => {
  const source = document();
  const { result } = renderHook(() => useConsoleCredentialSelectors({
    credentialPoolAutomation: null, credentialRefill: null,
    draftDocumentState: { document: source, invalid: false },
    draftAccountCatalog: buildRouteAccountCatalog(source),
  }));
  expect(result.current.credentialProviderOptions).toEqual([
    { id: "renamed-full", label: "Full-oauth" },
    { id: "express", label: "Express-api" },
    { id: "vertex-oauth", label: "Legacy-api" },
  ]);
});

it.each([true, false])("only overlays active summary identity from the same revision (%s)", (sameRevision) => {
  const source = document();
  const draft = document();
  delete (draft.providers[0] as Record<string, unknown>).auth_mode;
  const { result } = renderHook(() => useConsoleAccountSelectors({
    draftDocumentState: { document: draft }, draftMatchesActiveRevision: true,
    routeConfig: { routeConfig: { revision: { id: "r1", sequence: 1 }, source: "local", document: source,
      diagnostics: null, requiresRepair: false, secrets: [], mutationSupported: true } },
    accountGroupSummary: { routeConfigRevision: sameRevision ? "r1" : "r0", source: "local", accountGroups: [],
      providers: [{ id: "renamed-full", label: "Full", preset: presets[1], accountIds: ["stable-full"], supportedModels: [] }],
      accounts: [{ id: "stable-full", displayName: "Full", providerId: "renamed-full", providerLabel: "Full",
        providerPreset: presets[1], mode: "credential", enabled: true, supportedModels: [], groupIds: [] }] },
    credentialProbeResults: {}, runtimePressure: null, costOverview: null, requestAuditSummary: null,
    credentialModelStates: [], providerCredentialInventory: null, accountSearch: "", accountMembershipFilter: "all",
    accountEnabledFilter: "all", selectedAccountGroupFilter: "all", selectedProviderFilter: "all",
    locale: "zh-CN", pilotActionDialog: null, t: (zh) => zh,
  }));
  const visible = providerPoolTableRows(result.current.accountLedgerRows,
    { query: "", sort: "default", accessKinds: ["oauth"] });
  expect(visible.map((row) => row.accountId)).toEqual(sameRevision ? ["stable-full"] : []);
  expect((draft.providers[0] as Record<string, unknown>).auth_mode).toBeUndefined();
});
