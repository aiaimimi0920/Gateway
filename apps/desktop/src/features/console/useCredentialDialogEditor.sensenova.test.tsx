import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { CredentialDialogValue } from "./CredentialDialog";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

const base = "https://token.sensenova.cn/v1";
const draft: CredentialDialogValue = { providerId: "sensenova", credentialId: "stable", accountName: "", enabled: true,
  baseUrl: "", supportedModelsText: "manual", manualModelsText: "manual", fetchedModelsText: "",
  apiKeyOperation: "replace", apiKeyValue: "cpk-synthetic" };
function setup() {
  const active = routeDocument([{ id: "sensenova", preset: "sensenova-token-plan", base_url: base, explicit_credentials_only: true, credentials: [] }]);
  const error = vi.fn();
  const hook = renderHook(() => {
    const [document, setDocument] = useState(active);
    const [mode, setMode] = useState<"add" | "edit">("add");
    const [edits, setEdits] = useState<CredentialSecretEdit[]>([]);
    const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode, initialValue: null },
      setCredentialDialogState: vi.fn(), setCredentialSecretEdits: setEdits, setError: error, replaceEditorDocument: setDocument, t: (zh) => zh });
    return { editor, document, edits, setMode };
  });
  const patches = () => buildCredentialSecretPatches({ activeDocument: active, draftDocument: hook.result.current.document,
    activeSecrets: [], credentialSecretEdits: hook.result.current.edits });
  return { ...hook, error, patches };
}

it("stages a SenseNova account with a separate secret patch, keeping its stable ID on replacement", () => {
  const h = setup();
  act(() => h.result.current.editor.applyCredentialDialogValue(draft));
  expect(h.error).toHaveBeenLastCalledWith(null);
  expect(h.result.current.document.providers[0]).toMatchObject({ credentials: [{ id: "stable", model_selection: { fetched: [], manual: ["manual"] } }] });
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "cpk-synthetic" }]);
  act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...draft, apiKeyValue: "replacement-synthetic", modelCatalog: undefined }));
  expect(h.result.current.document.providers[0]).toMatchObject({ credentials: [{ id: "stable" }] });
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "replacement-synthetic" }]);
  expect(JSON.stringify(h.result.current.document)).not.toMatch(/cpk-synthetic|replacement-synthetic|discovery_job/);
  h.unmount();
});

it("preserves a pending SenseNova Key replacement when metadata is edited before autosave completes", () => {
  const h = setup();
  act(() => h.result.current.editor.applyCredentialDialogValue(draft));
  act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...draft, apiKeyOperation: "keep", apiKeyValue: "", accountName: "Renamed" }));
  expect(h.error).toHaveBeenLastCalledWith(null);
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "cpk-synthetic" }]);
  expect(h.result.current.document.providers[0]).toMatchObject({ credentials: [{ id: "stable", account_name: "Renamed" }] });
  h.unmount();
});
