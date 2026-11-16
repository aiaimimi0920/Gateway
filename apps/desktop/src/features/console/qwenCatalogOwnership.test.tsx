import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { routeDocument } from "./credentialDocumentTestFixtures";
import { acceptQwenCatalog } from "./qwenModelCatalog";
import { normalizeQwenCredentialDraft } from "./qwenCredentialDraft";

const base = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const catalog = { sourceUrl: base.replace("/compatible-mode/v1", "/api/v1/models"), accountBaseUrl: base,
  checkedAt: "2026-11-02T00:00:00Z", models: ["fetched"] };
const provider = () => ({ id: "ordinary", preset: "qwen-official-api", base_url: base,
  supported_models: ["fetched", "manual", "other"], credentials: [
    { id: "a", model_catalog: catalog, supported_models: ["fetched", "manual"] },
    { id: "b", supported_models: ["other"], model_selection: { manual: ["other"], fetched: [] } },
  ] });
it("metadata keep roundtrips a pre-selection native directory without losing ownership, routes or another account", () => {
  const active = routeDocument([provider()]); active.model_routes = [{ pattern: "fetched", provider_ids: ["ordinary"], priority: 42 },
    { pattern: "manual", provider_ids: ["ordinary"], enabled: false }];
  const initial = credentialDialogValueFromDocument(active, "ordinary", "a")!;
  const error = vi.fn(); const hook = renderHook(() => {
    const [document, setDocument] = useState(active);
    const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode: "edit", initialValue: initial },
      setCredentialDialogState: vi.fn(), setCredentialSecretEdits: vi.fn(), replaceEditorDocument: setDocument, setError: error, t: (zh) => zh });
    return { editor, document };
  });
  act(() => hook.result.current.editor.applyCredentialDialogValue({ ...initial, accountName: "metadata" }));
  expect(error).toHaveBeenLastCalledWith(null);
  expect((hook.result.current.document.providers[0] as Record<string, unknown>).credentials).toEqual([
    expect.objectContaining({ account_name: "metadata", model_catalog: catalog, model_selection: { manual: ["manual"], fetched: ["fetched"] } }), provider().credentials[1],
  ]);
  expect(hook.result.current.document.model_routes).toEqual(active.model_routes); hook.unmount();
});
it("only retains replacement evidence fetched with this exact Key, identity and public target", () => {
  const doc = routeDocument([provider()]); const previous = credentialDialogValueFromDocument(doc, "ordinary", "a")!;
  const replace = { ...previous, apiKeyOperation: "replace" as const, apiKeyValue: "synthetic-new" };
  expect(normalizeQwenCredentialDraft(provider(), replace, previous).modelCatalog).toBeUndefined();
  const fresh = { ...replace, modelCatalog: acceptQwenCatalog(provider(), replace, base, catalog) };
  expect(normalizeQwenCredentialDraft(provider(), fresh, previous).modelCatalog).toEqual(catalog);
  expect(normalizeQwenCredentialDraft(provider(), { ...fresh, apiKeyOperation: "keep", apiKeyValue: "" }, previous).modelCatalog).toBeUndefined();
  for (const changed of [{ ...fresh, apiKeyValue: "synthetic-other" }, { ...fresh, credentialId: "different" },
    { ...fresh, providerId: "different" }, { ...fresh, modelCatalog: structuredClone(fresh.modelCatalog) }]) {
    const normalized = normalizeQwenCredentialDraft(provider(), changed, previous);
    expect(normalized.modelCatalog).toBeUndefined(); expect(normalized.manualModelsText).toBe("manual");
    expect(normalized.fetchedModelsText).toBe("");
  }
});
