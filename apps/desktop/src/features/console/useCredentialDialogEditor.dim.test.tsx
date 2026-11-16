import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { dimCatalogFields, savedDimBinding, stageDimCatalog } from "./dimModelCatalog";
import { catalog, document as active } from "./dimCatalogTestFixtures";

it("publishes staged DIM declarations through the ordinary editor while preserving all OAuth secret Keep operations", () => {
  const initial = credentialDialogValueFromDocument(active, "dim", "account")!;
  const staged = stageDimCatalog(initial, catalog, savedDimBinding(active.providers[0], "account")!);
  const error = vi.fn();
  const h = renderHook(() => {
    const [document, setDocument] = useState<import("../../api/contracts").ConsoleRouteDocument>(active);
    const [edits, setEdits] = useState<CredentialSecretEdit[]>([]);
    const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode: "edit", initialValue: initial },
      setCredentialDialogState: vi.fn(), setCredentialSecretEdits: setEdits, setError: error, replaceEditorDocument: setDocument, t: (zh) => zh });
    return { editor, document, edits };
  });
  act(() => h.result.current.editor.applyCredentialDialogValue(staged));
  expect(error).toHaveBeenLastCalledWith(null); expect(h.result.current.edits).toEqual([]);
  const provider = h.result.current.document.providers[0] as Record<string, unknown>;
  const credential = (provider.credentials as Record<string, unknown>[])[0];
  expect(credential).toMatchObject({ id: "account", dim_catalog: catalog, dim_oauth: active.providers[0].credentials[0].dim_oauth,
    model_selection: { manual: ["manual-model"], fetched: ["opaque-model"] }, supported_models: ["manual-model", "opaque-model"] });
  expect(credential.model_catalog).toBeUndefined();
  const activeSecrets = ["api_key", "refresh_token"].map((field) => ({ path: `/providers/0/credentials/0/${field}`, configured: true }));
  expect(buildCredentialSecretPatches({ activeDocument: active, draftDocument: h.result.current.document, activeSecrets,
    credentialSecretEdits: h.result.current.edits })).toEqual(activeSecrets.map(({ path }) => ({ path, operation: "keep" })));
  expect(dimCatalogFields(provider, credentialDialogValueFromDocument(h.result.current.document, "dim", "account")!)).toEqual({ dim_catalog: catalog });
});
