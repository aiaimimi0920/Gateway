import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { applyModelSelection, editModelSources, modelSelectionSchema } from "./credentialModelSelection";
import { applyOfficialCatalog, type OfficialModelCatalog } from "./officialModelCatalog";
import { applyAccountDiscovery, type AccountDiscovery } from "./accountDiscovery";
import { applyChatgptCatalog } from "./chatgptModelCatalog";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildRouteAccountCatalog } from "./routeAccountCatalog";
import { buildRouteAccountCatalog as buildDocumentAccountCatalog } from "./routeDocumentAccountCatalog";

const catalog: OfficialModelCatalog = { sourceUrl: "https://api.poe.com/v1/models", checkedAt: "2026-10-26T04:00:00Z", models: ["shared", "new"] };
const document = { providers: [{ id: "poe", preset: "poe-openai", base_url: "https://api.poe.com/v1",
  supported_models: ["old", "shared", "manual", "other"], credentials: [
    { id: "one", account_name: "Legacy", enabled: false, api_key: null, base_url: "https://api.poe.com/v1",
      supported_models: ["old", "shared", "manual"], model_catalog: { ...catalog, models: ["old", "shared"] },
      model_selection: { fetched: ["old", "shared"], manual: ["manual", "shared"] } },
    { id: "two", supported_models: ["other", "old"] },
  ] }], model_routes: [{ pattern: "old", provider_ids: ["poe", "another"], priority: 42 },
  { pattern: "new", provider_ids: ["another"], priority: 99, enabled: false }], aliases: { custom: "manual" } };

it("persists editable ownership across reload, keeps overlaps and routes manual additions", () => {
  const before = structuredClone(document);
  const saved = applyModelSelection(document, "poe", "one", { fetched: ["shared"], manual: ["manual", "shared", "added"] });
  const value = credentialDialogValueFromDocument(JSON.parse(JSON.stringify(saved)), "poe", "one")!;
  expect(value).toMatchObject({ fetchedModelsText: "shared", manualModelsText: "manual\nshared\nadded",
    accountName: "Legacy", baseUrl: "https://api.poe.com/v1", enabled: false, apiKeyOperation: "keep" });
  expect(saved.model_routes).toContainEqual({ pattern: "added", provider_ids: ["poe"], priority: 10, enabled: true });
  expect(saved.model_routes[0]).toEqual(before.model_routes[0]); // another account still owns old
  expect(saved.aliases).toEqual(before.aliases);
  expect(document).toEqual(before);
});

it("refreshes only fetched ownership and never upgrades directory metadata to successful calls", () => {
  const saved = applyOfficialCatalog(document, "poe", "one", { ...catalog, models: ["new"] });
  expect(saved.providers[0]).toMatchObject({ credentials: [{ supported_models: ["manual", "shared", "new"],
    model_selection: { fetched: ["new"], manual: ["manual", "shared"] }, model_catalog: { ...catalog, models: ["new"] } }, { id: "two" }] });
  expect(saved.model_routes).toContainEqual({ pattern: "new", provider_ids: ["another", "poe"], priority: 99, enabled: false });
  expect(JSON.stringify(saved)).not.toContain("verified_models");
});

it("retains manual models for both generic protocol and ChatGPT catalog refresh", () => {
  const discovery: AccountDiscovery = { source_url: "https://fixture.test", api_base: "https://fixture.test/v1", binding: "fixture",
    protocol: "chat_completions", models: ["new"], verified_models: ["new"], checked_at: "now" };
  const generic = applyAccountDiscovery(document, "poe", "one", discovery);
  expect(generic.providers[0]).toMatchObject({ credentials: [{ supported_models: ["manual", "shared", "new"], discovery }, { id: "two" }] });
  const chatgpt = structuredClone(document); chatgpt.providers[0].preset = "chatgpt-codex-oauth-official-api";
  const saved = applyChatgptCatalog(chatgpt, "poe", "one", { clientVersion: "fixture", checkedAt: "now",
    models: [{ slug: "new", visibility: "list", supported_in_api: true }, { slug: "hidden", visibility: "hide", supported_in_api: true }] });
  expect(saved.providers[0]).toMatchObject({ credentials: [{ model_selection: { fetched: ["new"], manual: ["manual", "shared"] } }, { id: "two" }] });
});

it("saves edited subsets without reviving the full catalog, and supports an explicit empty selection", () => {
  const initial = credentialDialogValueFromDocument(document, "poe", "one")!;
  const replaceEditorDocument = vi.fn(); const setError = vi.fn();
  const { result } = renderHook(() => useCredentialDialogEditor({ editorText: JSON.stringify(document),
    credentialDialogState: { mode: "edit", initialValue: initial }, setCredentialDialogState: vi.fn(),
    setCredentialSecretEdits: vi.fn(), setError, replaceEditorDocument, t: (zh) => zh }));
  for (const fetched of ["new", ""]) {
    act(() => result.current.applyCredentialDialogValue(editModelSources({ ...initial, modelCatalog: catalog }, fetched, "")));
    const saved = replaceEditorDocument.mock.lastCall![0];
    expect(saved.providers[0].credentials[0]).toMatchObject({ model_selection: { fetched: fetched ? ["new"] : [], manual: [] },
      supported_models: fetched ? ["new"] : [], enabled: false, api_key: null, account_name: "Legacy", model_catalog: catalog });
    expect(saved.model_routes.some((entry: { pattern: string }) => entry.pattern === "shared")).toBe(false);
  }
  expect(setError).toHaveBeenLastCalledWith(null);
});

it("does not launch generative background discovery for explicit manual-only onboarding", () => {
  const doc = { providers: [{ id: "generic", base_url: "https://fixture.test", adapter: "openai_compatible", credentials: [] }], model_routes: [], aliases: {} };
  const replaceEditorDocument = vi.fn();
  const { result } = renderHook(() => useCredentialDialogEditor({ editorText: JSON.stringify(doc),
    credentialDialogState: { mode: "add", initialValue: null }, setCredentialDialogState: vi.fn(),
    setCredentialSecretEdits: vi.fn(), setError: vi.fn(), replaceEditorDocument, t: (zh) => zh }));
  act(() => result.current.applyCredentialDialogValue({ providerId: "generic", credentialId: "account", accountName: "", baseUrl: "",
    enabled: true, apiKeyOperation: "replace", apiKeyValue: "fixture-key", supportedModelsText: "manual", fetchedModelsText: "", manualModelsText: "manual" }));
  expect(replaceEditorDocument.mock.lastCall![0].providers[0].credentials[0]).toMatchObject({ supported_models: ["manual"],
    model_selection: { fetched: [], manual: ["manual"] } });
  expect(JSON.stringify(replaceEditorDocument.mock.lastCall![0])).not.toContain("discovery_job");
});

it("validates exact bounded IDs without silently accepting wildcard or control characters", () => {
  for (const id of ["*", "a?", "bad\tmodel", " padded", "模".repeat(86)]) {
    expect(modelSelectionSchema.safeParse({ fetched: [id], manual: [] }).success).toBe(false);
  }
  expect(modelSelectionSchema.safeParse({ fetched: [], manual: Array(2049).fill("a") }).success).toBe(false);
});

it("preserves effective inherited models when an old account is opened and saved", () => {
  const doc = { providers: [{ id: "generic", supported_models: ["inherited"], credentials: [{ id: "old", supported_models: [] }] }], model_routes: [], aliases: {} };
  const initial = credentialDialogValueFromDocument(doc, "generic", "old")!;
  expect(initial).toMatchObject({ supportedModelsText: "inherited", fetchedModelsText: "", manualModelsText: "inherited" });
  const explicit = structuredClone(doc);
  Object.assign(explicit.providers[0].credentials[0], { model_selection: { fetched: [], manual: [] } });
  expect(credentialDialogValueFromDocument(explicit, "generic", "old")!.supportedModelsText).toBe("");
});

it("seeds non-directory template models as declarations, not fetched or verified evidence", () => {
  const doc = { providers: [{ id: "search", preset: "tavily", supported_models: ["tavily-search", "tavily-fetch"] }], model_routes: [], aliases: {} };
  const setCredentialDialogState = vi.fn();
  const { result } = renderHook(() => useCredentialDialogEditor({ editorText: JSON.stringify(doc),
    credentialDialogState: null, setCredentialDialogState, setCredentialSecretEdits: vi.fn(), setError: vi.fn(), replaceEditorDocument: vi.fn(), t: (zh) => zh }));
  act(() => result.current.openAddCredentialDialog("search"));
  expect(setCredentialDialogState.mock.lastCall![0].initialValue).toMatchObject({ fetchedModelsText: "", manualModelsText: "tavily-search\ntavily-fetch" });
});

it("projects explicit selections to account cards without reviving provider inheritance", () => {
  const doc = { providers: [{ id: "generic", supported_models: ["provider-only"], credentials: [
    { id: "empty", supported_models: [], model_selection: { fetched: [], manual: [] } },
    { id: "selected", supported_models: ["stale"], model_selection: { fetched: ["fetched"], manual: ["manual"] } },
    { id: "legacy", supported_models: [] },
  ] }], model_routes: [], aliases: {} };
  for (const build of [buildRouteAccountCatalog, buildDocumentAccountCatalog]) {
    expect(Object.fromEntries(build(doc).accounts.map((account) => [account.id, account.supportedModels])))
      .toEqual({ empty: [], selected: ["manual", "fetched"], legacy: ["provider-only"] });
  }
});
