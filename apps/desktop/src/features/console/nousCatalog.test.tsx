import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { NOUS_TEMPLATE } from "./nousCatalog";
import { PROVIDER_CATALOG_TEMPLATES, providerDefinitionFromCatalogDraft } from "./providerCatalog";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { savedNousBinding, stageNousCatalog, nousModelCatalogSchema } from "./nousModelCatalog";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import type { ConsoleRouteDocument } from "../../api/contracts";
const active = { providers: [{ id: "pool", preset: "nous-portal-oauth", auth_mode: "nous_portal_oauth", explicit_credentials_only: true,
  base_url: NOUS_TEMPLATE.baseUrl, credentials: [{ id: "account", nous_portal: { client_id: "allowed-client", inference: "subscription", access_digest: "a".repeat(64) },
    supported_models: ["manual"], model_selection: { manual: ["manual"], fetched: [] } }] }], model_routes: [], aliases: {} };
it("registers independent Nous without invented models or automatic paid tests", () => {
  const t = NOUS_TEMPLATE; const workflow = defaultCatalogWorkflow(t); expect(PROVIDER_CATALOG_TEMPLATES).toContain(t);
  expect(workflow.testPolicy.automaticEnabled).toBe(false);
  const provider = providerDefinitionFromCatalogDraft(t, { templateId: t.id, providerId: "renamed", providerLabel: "Nous", vendorKey: t.vendorKey,
    vendorName: t.vendorName, baseUrl: t.baseUrl, supportedModels: [], workflow });
  expect(provider).toMatchObject({ preset: t.id, auth_mode: "nous_portal_oauth", explicit_credentials_only: true, supported_models: [] });
  expect(provider).not.toHaveProperty("api_key"); expect(provider).not.toHaveProperty("verified_models");
});
it("stages long exact IDs through the ordinary editor and retains OAuth secret Keep", () => {
  const initial = credentialDialogValueFromDocument(active, "pool", "account")!;
  const model = "org/" + "x".repeat(400);
  const staged = stageNousCatalog(initial, { models: [{ id: model }] }, savedNousBinding(active.providers[0], "account")!);
  const error = vi.fn();
  const h = renderHook(() => {
    const [document, setDocument] = useState<ConsoleRouteDocument>(active); const [edits, setEdits] = useState<CredentialSecretEdit[]>([]);
    const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode: "edit", initialValue: initial },
      setCredentialDialogState: vi.fn(), setCredentialSecretEdits: setEdits, setError: error, replaceEditorDocument: setDocument, t: (zh) => zh });
    return { editor, document, edits };
  });
  act(() => h.result.current.editor.applyCredentialDialogValue(staged));
  expect(error).toHaveBeenLastCalledWith(null);
  const c = (h.result.current.document.providers[0] as typeof active.providers[0]).credentials[0];
  expect(c.model_selection).toEqual({ manual: ["manual"], fetched: [model] });
  expect(c.nous_portal).toEqual(active.providers[0].credentials[0].nous_portal);
  const secrets = ["api_key", "refresh_token"].map((field) => ({ path: `/providers/0/credentials/0/${field}`, configured: true }));
  expect(buildCredentialSecretPatches({ activeDocument: active, draftDocument: h.result.current.document, activeSecrets: secrets,
    credentialSecretEdits: h.result.current.edits })).toEqual(secrets.map(({ path }) => ({ path, operation: "keep" })));
  const empty = stageNousCatalog(staged, { models: [] }, savedNousBinding(active.providers[0], "account")!);
  expect(empty.supportedModelsText).toBe("manual");
});
it("rejects lossy directory IDs, duplicates and foreign account staging", () => {
  const initial = credentialDialogValueFromDocument(active, "pool", "account")!; const binding = savedNousBinding(active.providers[0], "account")!;
  for (const id of ["a,b", "*", "x".repeat(513)]) expect(() => stageNousCatalog(initial, { models: [{ id }] }, binding)).toThrow();
  expect(nousModelCatalogSchema.safeParse({ models: [{ id: "a" }, { id: "a" }] }).success).toBe(false);
  expect(() => stageNousCatalog({ ...initial, credentialId: "other" }, { models: [] }, binding)).toThrow();
});
