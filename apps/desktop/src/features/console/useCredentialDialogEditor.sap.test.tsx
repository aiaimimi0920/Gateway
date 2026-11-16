import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";
import { sapBase, sapDraft, sapKey, sapProvider } from "./sapAiCoreTestFixtures";

function setup() {
  const active = routeDocument([sapProvider]); const error = vi.fn();
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

it("creates a SAP account using the deployment ID and separate service-key patch, then replaces with stable ID", () => {
  const h = setup();
  act(() => h.result.current.editor.applyCredentialDialogValue(sapDraft));
  expect(h.error).toHaveBeenLastCalledWith(null);
  expect(h.result.current.document.providers[0]).toMatchObject({ credentials: [{ id: "stable", base_url: sapBase,
    model_selection: { fetched: [], manual: ["deployment1"] } }] });
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: sapKey }]);
  act(() => h.result.current.setMode("edit"));
  const replacement = sapKey.replace("synthetic-secret", "replacement-secret");
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...sapDraft, apiKeyValue: replacement }));
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: replacement }]);
  expect(h.result.current.document.providers[0]).toMatchObject({ credentials: [{ id: "stable" }] });
  expect(JSON.stringify(h.result.current.document)).not.toMatch(/synthetic-secret|replacement-secret|discovery_job/);
  h.unmount();
});

it("retains a staged SAP secret during metadata Keep before autosave", () => {
  const h = setup();
  act(() => h.result.current.editor.applyCredentialDialogValue(sapDraft));
  act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...sapDraft, apiKeyOperation: "keep", apiKeyValue: "", accountName: "Renamed" }));
  expect(h.error).toHaveBeenLastCalledWith(null);
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: sapKey }]);
  expect(h.result.current.document.providers[0]).toMatchObject({ credentials: [{ id: "stable", account_name: "Renamed" }] });
  h.unmount();
});

it.each(["{bad", "{}", sapKey.replace("synthetic.authentication.eu10", "evil.authentication.eu10.evil"),
  sapKey.replace("eu-central-1", "us-east-1")])("rejects incomplete or mismatched service keys without staging", (apiKeyValue) => {
  const h = setup();
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...sapDraft, apiKeyValue }));
  expect(h.error).toHaveBeenLastCalledWith(expect.any(String));
  expect(h.patches()).toEqual([]);
  expect(h.result.current.document.providers[0]).toMatchObject({ credentials: [] });
  h.unmount();
});
