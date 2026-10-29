import { expect, it } from "vitest";
import { buildAccountLedgerSections } from "./accountLedgerSections";
import { buildConsoleTelemetrySnapshot } from "./telemetry";
import { buildAccountLedgerRows, buildRouteAccountCatalog } from "./accountManagementViewModel";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { applyGeminiGeneratedDraftsToDocument } from "./geminiCredentialDraft";
import { buildCredentialSecretPatches } from "./credentialDocument";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { consoleGeminiAuthSessionResponseSchema } from "../../api/schemas/credentials";

const snapshot = buildConsoleTelemetrySnapshot({ pressure: null, costOverview: null,
  credentialModelStates: [], credentialInventory: null, requestAuditSummary: null });

it("keeps captured Google Cookie material in the existing secret patch owner", () => {
  const { session } = consoleGeminiAuthSessionResponseSchema.parse({ session: {
    id: "session", providerId: "web", targetFamily: "gemini-web", status: "succeeded",
    message: "captured", createdAt: "now", updatedAt: "now", generatedDrafts: [{
      providerId: "web", credential: { id: "account", api_key: "", headers: { Cookie: "" } },
      secretEdits: [{ field: "headers/Cookie", operation: "replace", value: "SID=synthetic-cookie" }],
    }],
  }});
  const result = applyGeminiGeneratedDraftsToDocument({providers: [{id: "web", preset: "gemini-web-chat",
    credentials: []}], model_routes: [], aliases: {}}, session.generatedDrafts);
  expect(result.secretEdits).toContainEqual({providerId: "web", credentialId: "account",
    field: "headers/Cookie", operation: "replace", value: "SID=synthetic-cookie"});
  expect(JSON.stringify(result.document)).not.toContain("synthetic-cookie");
});

it("commits captured Web credentials only through secret patches, not even empty inline placeholders", () => {
  const before: ConsoleRouteDocument = {providers: [{id: "web", preset: "gemini-web-chat",
    credentials: []}], model_routes: [], aliases: {}};
  const drafts = [{providerId: "web", credential: {id: "account", api_key: "", auth_token: "",
    headers: {Cookie: "", Referer: "https://gemini.google.com/app"},
    extra_body: {accessToken: "synthetic-access", sessionId: "synthetic-session", language: "zh-CN"}},
    secretEdits: [
      {field: "api_key" as const, operation: "replace" as const, value: "synthetic-primary"},
      {field: "auth_token" as const, operation: "replace" as const, value: "synthetic-secondary"},
      {field: "headers/Cookie" as const, operation: "replace" as const, value: "SID=synthetic-cookie"},
    ]}];
  const original = structuredClone(drafts);
  const result = applyGeminiGeneratedDraftsToDocument(before, drafts);
  const account = ((result.document.providers[0] as Record<string, unknown>).credentials as Record<string, unknown>[])[0];
  for (const field of ["api_key", "auth_token"]) expect(account).not.toHaveProperty(field);
  expect(account.headers).toEqual({Referer: "https://gemini.google.com/app"});
  expect(account.extra_body).toEqual({language: "zh-CN"});
  const patches = buildCredentialSecretPatches({activeDocument: before, draftDocument: result.document,
    activeSecrets: [], credentialSecretEdits: result.secretEdits});
  expect(patches.map(patch => patch.path)).toEqual([
    "/providers/0/credentials/0/api_key", "/providers/0/credentials/0/auth_token",
    "/providers/0/credentials/0/extra_body/accessToken", "/providers/0/credentials/0/extra_body/sessionId",
    "/providers/0/credentials/0/headers/Cookie",
  ]);
  expect(patches.every(patch => patch.operation === "replace" && Boolean(patch.value))).toBe(true);
  expect(JSON.stringify(result.document)).not.toContain("synthetic-");
  expect(drafts).toEqual(original);
});

it("keeps a new Gemini Web pool on its own Web login capture flow", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === "gemini-web")!;
  const draft = { templateId: template.id, providerId: template.providerId, providerLabel: template.labelZh,
    vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl: template.baseUrl,
    supportedModels: [], workflow: defaultCatalogWorkflow(template), secrets: {} };
  const document = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, draft);
  expect(document.providers[0]).toMatchObject({ id: "gemini-web", preset: "gemini-web-chat",
    explicit_credentials_only: true, credentials: [],
    call_flow: { call_adapter: "auto", fingerprint_adapter: "session", special_adapter: "gemini_web" } });
  const rows = buildAccountLedgerRows(buildRouteAccountCatalog(document), {});
  expect(rows).toEqual([]);
  const sections = buildAccountLedgerSections(document, rows, new Map(), snapshot, "zh-CN");
  expect(sections).toHaveLength(1);
  expect(sections[0]).toMatchObject({ providerId: "gemini-web", manualAddFamily: "gemini-web",
    directAccounts: [], hasExplicitAccounts: false });
});

it("preserves the selected primary provider's capture family in legacy merged Gemini pools", () => {
  for (const canvas of [false, true]) {
    const providers = [{ id: "custom-web", preset: "gemini-web-chat", explicit_credentials_only: true, credentials: [] },
      ...(canvas ? [{ id: "gemini-canvas-chat", preset: "gemini-canvas-chat", explicit_credentials_only: true, credentials: [] }] : [])];
    const document = { providers, model_routes: [], aliases: {} };
    const sections = buildAccountLedgerSections(document, [], new Map(), snapshot, "zh-CN");
    expect(sections[0]).toMatchObject(canvas
      ? { providerId: "gemini-canvas-chat", manualAddFamily: "gemini-canvas-chat" }
      : { providerId: "custom-web", manualAddFamily: "gemini-web" });
  }
});

it("registers the Web session alias after capture without claiming a live model directory", () => {
  const before = { providers: [{ id: "saved-web", preset: "gemini-web-chat", explicit_credentials_only: true,
    credentials: [] }], model_routes: [], aliases: {} };
  const imported = applyGeminiGeneratedDraftsToDocument(before, [{ providerId: "saved-web",
    credential: { id: "web-account", api_key: "", auth_token: "", extra_body: { authUser: "0" } },
    secretEdits: [{ field: "api_key", operation: "replace", value: "synthetic-primary-secret" }] }]);
  expect(imported.document.providers[0]).toMatchObject({ supported_models: ["gemini-web-chat"],
    credentials: [{ id: "web-account", supported_models: ["gemini-web-chat"],
      model_selection: { fetched: [], manual: ["gemini-web-chat"] } }] });
  expect(imported.document.model_routes).toContainEqual(expect.objectContaining({
    pattern: "gemini-web-chat", provider_ids: ["saved-web"] }));
  expect(JSON.stringify(imported.document)).not.toMatch(/synthetic-primary-secret|verified_models|model_catalog/);
  expect(before.providers[0].credentials).toEqual([]);
});

it("preserves model sources, disabled routes and other accounts when a Web draft is reapplied", () => {
  for (const selection of [undefined, { fetched: ["saved-fetched"], manual: ["saved-manual"] },
    { fetched: [], manual: [] }]) {
    const before: ConsoleRouteDocument = { providers: [{ id: "saved-web", preset: "gemini-web-chat-modular",
      supported_models: ["saved-manual", "saved-fetched", "other-account-model"], credentials: [
        { id: "web-account", supported_models: ["saved-manual", "saved-fetched"],
          ...(selection ? { model_selection: selection } : {}) },
        { id: "other-account", supported_models: ["other-account-model"], enabled: false },
      ] }, { id: "unrelated", supported_models: ["unrelated-model"], credentials: [] }], model_routes: [
        { pattern: "saved-manual", provider_ids: ["saved-web", "unrelated"], priority: 7, enabled: false },
        { pattern: "other-account-model", provider_ids: ["saved-web"], enabled: false },
        { pattern: "unrelated-model", provider_ids: ["unrelated"], enabled: true },
      ], aliases: { "saved-alias": "saved-manual" } };
    const original = structuredClone(before);
    const drafts = [{ providerId: "saved-web", credential: { id: "web-account", extra_body: { authUser: "1" } },
      secretEdits: [] }];
    const first = applyGeminiGeneratedDraftsToDocument(before, drafts).document;
    const second = applyGeminiGeneratedDraftsToDocument(first, drafts).document;
    expect(second).toEqual(first);
    expect(before).toEqual(original);
    expect(first.providers[1]).toEqual(before.providers[1]);
    const credentials = (first.providers[0] as Record<string, unknown>).credentials as Record<string, unknown>[];
    expect(credentials[0].model_selection).toEqual(selection ?? {
      fetched: [], manual: ["saved-manual", "saved-fetched"] });
    expect(credentials[1]).toEqual(((before.providers[0] as Record<string, unknown>).credentials as unknown[])[1]);
    expect(first.model_routes).toEqual(expect.arrayContaining(before.model_routes));
    expect(first.aliases).toEqual(before.aliases);
    expect(JSON.stringify(first)).not.toMatch(/gemini-web-chat"|verified_models|model_catalog/);
  }
});
