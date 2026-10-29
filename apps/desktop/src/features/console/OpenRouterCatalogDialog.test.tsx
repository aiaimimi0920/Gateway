import { act, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog, type CredentialDialogValue } from "./CredentialDialog";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildAccountLedgerSections } from "./accountLedgerSections";
import { buildAccountLedgerRows, buildRouteAccountCatalog } from "./accountManagementViewModel";
import { buildConsoleTelemetrySnapshot } from "./telemetry";
import { applyOfficialCatalog, type OfficialModelCatalog } from "./officialModelCatalog";
import { editModelSources } from "./credentialModelSelection";

const catalog: OfficialModelCatalog = { sourceUrl: "https://openrouter.ai/api/v1/models",
  checkedAt: "2026-10-26T04:00:00Z", models: ["vendor/one", "vendor/two"] };
const document = { providers: [{ id: "or", preset: "openrouter-openai", base_url: "https://openrouter.ai/api/v1",
  supported_models: ["manual", "old"], credentials: [{ id: "one", enabled: false, api_key: null,
    supported_models: ["manual", "old"], model_catalog: { ...catalog, models: ["old"] } }] }],
  model_routes: [{ pattern: "old", provider_ids: ["or", "other"], priority: 42 },
    { pattern: "vendor/two", provider_ids: ["other"], enabled: false, priority: 99 }], aliases: {} };
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

function dialog(mode: "add" | "edit", discover = vi.fn().mockResolvedValue(catalog), initialValue?: CredentialDialogValue) {
  const onSubmit = vi.fn(); const onDiscover = vi.fn();
  const props = { open: true, mode, providerOptions: [{ id: "or", label: "OpenRouter" }], existingCredentialIds: [],
    locked: false, hasSecretAccess: true, onOpenChange: vi.fn(), onRequestSecretAccess: vi.fn(), onSubmit,
    catalogProviderKind: () => "openrouter" as const, onDiscoverProviderModels: discover, onDiscover, initialValue };
  return { ...render(<UiLocaleProvider><CredentialDialog {...props} /></UiLocaleProvider>), onSubmit, onDiscover, discover };
}

it("reads OpenRouter catalog on import and never invokes generic generation discovery", async () => {
  const ui = dialog("add");
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(ui.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ modelCatalog: catalog })));
  expect(ui.discover).toHaveBeenCalledWith({ providerId: "or", apiKey: "fixture-key" }, expect.any(AbortSignal));
  expect(ui.onDiscover).not.toHaveBeenCalled();
});

it("upgrades a seed-only existing account using its stored Key without changing enabled state", async () => {
  const initial = { ...credentialDialogValueFromDocument(document, "or", "one")!, modelCatalog: undefined };
  const ui = dialog("edit", vi.fn().mockResolvedValue(catalog), initial);
  fireEvent.click(screen.getByRole("button", { name: "拉取模型" }));
  await waitFor(() => expect(screen.getByLabelText("拉取模型列表")).toHaveValue(catalog.models.join("\n")));
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(ui.discover).toHaveBeenCalledWith({ credentialId: "one" }, expect.any(AbortSignal)));
  await waitFor(() => expect(ui.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyOperation: "keep", apiKeyValue: "", enabled: false, modelCatalog: catalog })));
});

it("preserves the last-good catalog on failed explicit refresh", async () => {
  const initial = credentialDialogValueFromDocument(document, "or", "one")!;
  const ui = dialog("edit", vi.fn().mockRejectedValue(new Error("offline")), initial);
  fireEvent.click(screen.getByRole("button", { name: "拉取模型" }));
  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("offline"));
  expect(screen.getByRole("status")).toHaveTextContent("目录已发现 1");
  expect(ui.onSubmit).not.toHaveBeenCalled();
});

it("cancels an in-flight import when the dialog unmounts", async () => {
  let resolve!: (result: OfficialModelCatalog) => void;
  const ui = dialog("add", vi.fn(() => new Promise((done) => { resolve = done; })));
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  const signal = ui.discover.mock.calls[0][1] as AbortSignal;
  ui.unmount();
  await act(async () => { resolve(catalog); });
  expect(signal.aborted).toBe(true);
  expect(ui.onSubmit).not.toHaveBeenCalled();
});

it("persists metadata, manual models and route policy without touching secrets or enabling accounts", () => {
  const initial = credentialDialogValueFromDocument(document, "or", "one")!;
  const replaceEditorDocument = vi.fn(); const setCredentialSecretEdits = vi.fn();
  const hook = renderHook(() => useCredentialDialogEditor({ editorText: JSON.stringify(document),
    credentialDialogState: { mode: "edit", initialValue: initial }, setCredentialDialogState: vi.fn(),
    setCredentialSecretEdits, setError: vi.fn(), replaceEditorDocument, t: (zh) => zh }));
  act(() => hook.result.current.applyCredentialDialogValue(editModelSources({ ...initial, modelCatalog: catalog }, catalog.models.join("\n"), "manual")));
  const saved = replaceEditorDocument.mock.calls[0][0];
  expect(saved.providers[0]).toMatchObject({ supported_models: ["manual", ...catalog.models],
    credentials: [{ id: "one", api_key: null, enabled: false, supported_models: ["manual", ...catalog.models], model_catalog: catalog }] });
  expect(saved.model_routes).toContainEqual({ pattern: "old", provider_ids: ["other"], priority: 42 });
  expect(saved.model_routes).toContainEqual({ pattern: "vendor/two", provider_ids: ["other", "or"], enabled: false, priority: 99 });
  expect(setCredentialSecretEdits.mock.calls[0][0]([])).toEqual([]);
  expect(JSON.stringify(saved)).not.toContain("verified_models");
  expect(applyOfficialCatalog(saved, "or", "one", catalog)).toEqual(saved);
});

it("carries unverified directory models and refresh support to OpenRouter account and provider cards", () => {
  const doc = applyOfficialCatalog(document, "or", "one", catalog);
  const rows = buildAccountLedgerRows(buildRouteAccountCatalog(doc), {});
  const snapshot = buildConsoleTelemetrySnapshot({ pressure: null, costOverview: null, credentialModelStates: [], credentialInventory: null, requestAuditSummary: null });
  const sections = buildAccountLedgerSections(doc, rows, new Map(), snapshot, "zh-CN");
  expect(sections[0].modelTraffic?.catalogModels).toEqual(catalog.models);
  expect(sections[0].directAccounts[0]).toMatchObject({ discoverySupported: true });
  expect(sections[0].directAccounts[0].modelTraffic?.models.size).toBe(0);
});

it("rejects cross-provider sources and unofficial override origins without changing the document", () => {
  const before = structuredClone(document);
  expect(() => applyOfficialCatalog(document, "or", "one", { ...catalog, sourceUrl: "https://integrate.api.nvidia.com/v1/models" })).toThrow();
  const custom = structuredClone(document);
  Object.assign(custom.providers[0].credentials[0], { base_url: "https://evil.invalid" });
  expect(() => applyOfficialCatalog(custom, "or", "one", catalog)).toThrow();
  expect(document).toEqual(before);
});
