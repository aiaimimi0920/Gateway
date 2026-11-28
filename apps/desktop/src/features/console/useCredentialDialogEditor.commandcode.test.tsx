import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { CredentialDialogValue } from "./CredentialDialog";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildCredentialSecretPatches, updateExplicitCredential, type CredentialSecretEdit } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";

const base = "https://api.commandcode.ai/provider/v1";
const draft: CredentialDialogValue = { providerId: "commandcode", credentialId: "stable", accountName: "", enabled: true,
  baseUrl: "", supportedModelsText: "manual", manualModelsText: "manual", fetchedModelsText: "",
  apiKeyOperation: "replace", apiKeyValue: "cpk-synthetic" };
function setup() {
  const active = routeDocument([{ id: "commandcode", preset: "commandcode-api", base_url: base, explicit_credentials_only: true, credentials: [] }]);
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

it("stages a CommandCode account with a separate secret patch, keeping its stable ID on replacement", () => {
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

it("preserves a pending CommandCode Key replacement when metadata is edited before autosave completes", () => {
  const h = setup();
  act(() => h.result.current.editor.applyCredentialDialogValue(draft));
  act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...draft, apiKeyOperation: "keep", apiKeyValue: "", accountName: "Renamed" }));
  expect(h.error).toHaveBeenLastCalledWith(null);
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "cpk-synthetic" }]);
  expect(h.result.current.document.providers[0]).toMatchObject({ credentials: [{ id: "stable", account_name: "Renamed" }] });
  h.unmount();
});

it("saves and reopens account ZDR without replacing the existing Key", () => {
  const h = setup();
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...draft, commandcodeZdr: "required" }));
  expect(h.result.current.document.providers[0]).toMatchObject({ credentials: [{ headers: { "x-cmd-zdr": "1" } }] });
  const reopened = credentialDialogValueFromDocument(h.result.current.document, "commandcode", "stable")!;
  expect(reopened.commandcodeZdr).toBe("required");
  act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...reopened, commandcodeZdr: "inherit" }));
  expect(h.result.current.document.providers[0]).toMatchObject({ credentials: [{ headers: {} }] });
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "cpk-synthetic" }]);
  expect(h.error).toHaveBeenLastCalledWith(null);
  h.unmount();
});

it("saves and reads back the per-account ceiling and can clear it", () => {
  const h = setup();
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...draft, maxConcurrentRequestsText: "3" }));
  expect(h.error).toHaveBeenLastCalledWith(null);
  expect(h.result.current.document.providers[0]).toMatchObject({ credentials: [{ id: "stable", max_concurrent_requests: 3 }] });
  expect(credentialDialogValueFromDocument(h.result.current.document, "commandcode", "stable")?.maxConcurrentRequestsText).toBe("3");
  act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({
    ...draft, maxConcurrentRequestsText: "", apiKeyOperation: "keep", apiKeyValue: ""
  }));
  expect(JSON.stringify(h.result.current.document)).not.toContain("max_concurrent_requests");
  h.unmount();
});

it("edits one zzz-api account limit without overwriting another manual value", () => {
  const active = routeDocument([{ id: "zzz-api", base_url: "https://fixture.invalid",
    credentials: [{ id: "account-a", max_concurrent_requests: 5 }, { id: "account-b", max_concurrent_requests: 7 }] }]);
  const next = updateExplicitCredential(active, { providerId: "zzz-api", credentialId: "account-a" },
    { max_concurrent_requests: 3 });
  expect(next.providers[0]).toMatchObject({ credentials: [
    { id: "account-a", max_concurrent_requests: 3 },
    { id: "account-b", max_concurrent_requests: 7 }
  ] });
  expect(active.providers[0]).toMatchObject({ credentials: [
    { id: "account-a", max_concurrent_requests: 5 },
    { id: "account-b", max_concurrent_requests: 7 }
  ] });
});
