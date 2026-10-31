import { expect, it } from "vitest";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { buildAccountLedgerSections } from "./accountLedgerSections";
import { buildConsoleTelemetrySnapshot } from "./telemetry";
import { applyGeminiGeneratedDraftsToDocument } from "./geminiCredentialDraft";
import { resolveGeminiLogicalChannel } from "./accountManagementViewModel";

const snapshot = buildConsoleTelemetrySnapshot({ pressure: null, costOverview: null,
  credentialModelStates: [], credentialInventory: null, requestAuditSummary: null });
const empty = () => ({ providers: [], model_routes: [], aliases: {} });

it("offers distinct Gemini Web and true Canvas templates without creating credentials or routes", () => {
  const web = PROVIDER_CATALOG_TEMPLATES.find(template => template.id === "gemini-web")!;
  const canvas = PROVIDER_CATALOG_TEMPLATES.find(template => template.id === "gemini-canvas")!;
  expect(web.preset).toBe("gemini-web-chat");
  expect(canvas.preset).toBe("gemini-canvas-browser-relay");
  const draft = { templateId: canvas.id, providerId: "custom-canvas", providerLabel: canvas.labelZh,
    vendorKey: canvas.vendorKey, vendorName: canvas.vendorName, baseUrl: canvas.baseUrl,
    supportedModels: [], workflow: defaultCatalogWorkflow(canvas), secrets: {} };
  const document = addCatalogProvider(empty(), canvas, draft);
  expect(document.providers[0]).toMatchObject({ id: "custom-canvas", preset: "gemini-canvas-browser-relay",
    explicit_credentials_only: true, credentials: [],
    call_flow: { call_adapter: "auto", fingerprint_adapter: "session", special_adapter: "gemini_canvas" } });
  expect(document.model_routes).toEqual([]);
  const sections = buildAccountLedgerSections(document, [], new Map(), snapshot, "zh-CN");
  expect(sections[0]).toMatchObject({ providerId: "custom-canvas", manualAddFamily: "gemini-canvas" });
});

it("does not merge new Web and Canvas pools or change their credential capture protocols", () => {
  const document = { ...empty(), providers: [
    { id: "web-custom", preset: "gemini-web-chat", credentials: [], explicit_credentials_only: true },
    { id: "canvas-custom", preset: "gemini-canvas-browser-relay", credentials: [], explicit_credentials_only: true },
  ] };
  const sections = buildAccountLedgerSections(document, [], new Map(), snapshot, "zh-CN");
  expect(sections).toHaveLength(2);
  expect(sections[0]).toMatchObject({ providerId: "web-custom", providerLabel: "gemini-web", manualAddFamily: "gemini-web" });
  expect(sections[1]).toMatchObject({ providerId: "canvas-custom", providerLabel: "gemini-canvas", manualAddFamily: "gemini-canvas" });
});

it("uses the actual preset rather than a misleading provider name to select Web or Canvas", () => {
  expect(resolveGeminiLogicalChannel("gemini-canvas", "gemini-web-chat")?.key).toBe("logical:gemini");
  expect(resolveGeminiLogicalChannel("gemini-web", "gemini-canvas-browser-relay")?.key).toBe("logical:gemini-canvas");
});

it("registers working Canvas text, image and speech defaults without claiming model discovery", () => {
  const defaults = ["gemini-3-flash-preview", "gemini-2.5-flash-image", "gemini-2.5-flash-preview-tts"];
  const before = { ...empty(), providers: [{ id: "custom-canvas", preset: "gemini-canvas-browser-relay", credentials: [] }] };
  const result = applyGeminiGeneratedDraftsToDocument(before, [{ providerId: "custom-canvas",
    credential: { id: "canvas-account", runtime_state_object_key: "credential-runtime/fixture/state.json",
      extra_body: { shareId: "fixture" } }, secretEdits: [] }]);
  expect(result.document.providers[0]).toMatchObject({ credentials: [{ id: "canvas-account",
    runtime_state_object_key: "credential-runtime/fixture/state.json", supported_models: defaults,
    model_selection: { fetched: [], manual: defaults } }] });
  for (const model of defaults) expect(result.document.model_routes).toContainEqual(expect.objectContaining({
    pattern: model, provider_ids: ["custom-canvas"] }));
  expect(result.secretEdits).toEqual([]);
  expect(JSON.stringify(result.document)).not.toMatch(/apiKeys|model_catalog|verified_models|gemini-canvas-chat|music|video/);
  expect(before.providers[0].credentials).toEqual([]);
});

it("preserves an explicitly empty Canvas selection instead of adding media routes on refresh", () => {
  const before = { ...empty(), providers: [{ id: "canvas", preset: "gemini-canvas-browser-relay",
    credentials: [{ id: "account", supported_models: [], model_selection: { fetched: [], manual: [] } }] }] };
  const result = applyGeminiGeneratedDraftsToDocument(before, [{ providerId: "canvas",
    credential: { id: "account", runtime_state_object_key: "new-state" }, secretEdits: [] }]);
  expect(result.document.providers[0]).toMatchObject({ credentials: [{
    model_selection: { fetched: [], manual: [] }, supported_models: [] }] });
  expect(result.document.model_routes).toEqual([]);
});

it("retains manual Canvas models and disabled routes when refreshing browser state", () => {
  const before = { ...empty(), providers: [{ id: "custom-canvas", preset: "gemini-canvas-browser-relay",
    credentials: [{ id: "account", runtime_state_object_key: "old-state", supported_models: ["saved"],
      model_selection: { fetched: [], manual: ["saved"] } }] }],
    model_routes: [{ pattern: "saved", provider_ids: ["custom-canvas"], enabled: false }] };
  const result = applyGeminiGeneratedDraftsToDocument(before, [{ providerId: "custom-canvas",
    credential: { id: "account", runtime_state_object_key: "new-state" }, secretEdits: [] }]);
  expect(result.document.providers[0]).toMatchObject({ credentials: [{ runtime_state_object_key: "new-state",
    model_selection: { fetched: [], manual: ["saved"] } }] });
  expect(result.document.model_routes).toEqual(before.model_routes);
});
