import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import type { CredentialDialogValue } from "./CredentialDialog";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { routeDocument } from "./credentialDocumentTestFixtures";

const base = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const draft = (apiKeyOperation: CredentialDialogValue["apiKeyOperation"]): CredentialDialogValue => ({ providerId: "renamed", credentialId: "a",
  accountName: "", enabled: true, baseUrl: "", supportedModelsText: "manual", manualModelsText: "manual", fetchedModelsText: "",
  apiKeyOperation, apiKeyValue: apiKeyOperation === "replace" ? "synthetic-new-key" : "" });
function setup(active = routeDocument([{ id: "renamed", preset: "qwen-official-api", base_url: base, explicit_credentials_only: true, credentials: [] }])) {
  const error = vi.fn(); const dialog = vi.fn();
  const hook = renderHook(() => {
    const [document, setDocument] = useState(active);
    const [mode, setMode] = useState<"add" | "edit">("add");
    const [edits, setEdits] = useState<CredentialSecretEdit[]>([]);
    const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode, initialValue: null },
      setCredentialDialogState: dialog, setCredentialSecretEdits: setEdits, setError: error, replaceEditorDocument: setDocument, t: (zh) => zh });
    return { editor, document, edits, setMode, setDocument };
  });
  const patches = (document?: ConsoleRouteDocument, strictSessionBindings = true) => buildCredentialSecretPatches({ activeDocument: active,
    draftDocument: document ?? hook.result.current.document, activeSecrets: [], credentialSecretEdits: hook.result.current.edits, strictSessionBindings });
  return { ...hook, active, error, dialog, patches };
}

it("does not promote another account's fetched directory into a new Qwen account's manual choices", () => {
  const active = routeDocument([{ id: "renamed", preset: "qwen-official-api", base_url: base, supported_models: ["other-account-fetched"],
    credentials: [{ id: "b", supported_models: ["other-account-fetched"], model_selection: { fetched: ["other-account-fetched"], manual: [] } }] }]);
  const h = setup(active);
  act(() => h.result.current.editor.openAddCredentialDialog("renamed"));
  expect(h.dialog).toHaveBeenCalledWith(expect.objectContaining({ mode: "add", initialValue: expect.objectContaining({
    supportedModelsText: "", apiKeyOperation: "replace", apiKeyValue: "" }) }));
  expect(JSON.stringify(h.dialog.mock.calls)).not.toContain("other-account-fetched");
  h.unmount();
});

it("keeps the original staged replacement and binding through a later metadata-only keep", () => {
  const h = setup();
  act(() => h.result.current.editor.applyCredentialDialogValue(draft("replace")));
  expect(h.error).toHaveBeenLastCalledWith(null);
  const staged = structuredClone(h.result.current.edits);
  expect(staged).toHaveLength(1);
  expect(staged[0]).toMatchObject({ operation: "replace", qwenBinding: { product: "qwen-official-api", baseUrl: base } });
  act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...draft("keep"), accountName: "Metadata only" }));
  expect(h.error).toHaveBeenLastCalledWith(null);
  expect(h.result.current.edits).toEqual(staged);
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-new-key" }]);
  expect(JSON.stringify(h.result.current.document)).not.toContain("synthetic-new-key");
  expect(JSON.stringify(h.result.current.document)).not.toContain("discovery_job");
  h.unmount();
});

it("does not reauthorize a staged Key when its inherited parent target changes", () => {
  const h = setup();
  act(() => h.result.current.editor.applyCredentialDialogValue(draft("replace")));
  const staged = structuredClone(h.result.current.edits);
  act(() => { h.result.current.setMode("edit"); h.result.current.setDocument((document) => {
    const next = structuredClone(document); (next.providers[0] as Record<string, unknown>).base_url = base.replace("cn-beijing", "eu-central-1"); return next;
  }); });
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...draft("keep"), accountName: "Still metadata only" }));
  expect(h.result.current.edits).toEqual(staged);
  expect(() => h.patches()).toThrow("目标已变化");
  expect(h.patches(undefined, false)).toEqual([]);
  h.unmount();
});

it("clear replaces an earlier staged Key instead of emitting or rebinding its old value", () => {
  const h = setup();
  act(() => h.result.current.editor.applyCredentialDialogValue(draft("replace")));
  act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...draft("clear"), baseUrl: base.replace("cn-beijing", "eu-central-1") }));
  expect(h.error).toHaveBeenLastCalledWith(null);
  expect(h.result.current.edits).toEqual([{ providerId: "renamed", credentialId: "a", field: "api_key", operation: "clear" }]);
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "clear" }]);
  h.unmount();
});

it("rejects keep retargeting before mutating either document or staged secrets", () => {
  const active = routeDocument([{ id: "renamed", preset: "qwen-official-api", base_url: base,
    credentials: [{ id: "a", api_key: "[REDACTED]", supported_models: ["manual"] }] }]);
  const h = setup(active); act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...draft("keep"), baseUrl: base.replace("workspace-a", "workspace-b") }));
  expect(h.error).toHaveBeenLastCalledWith(expect.stringContaining("目标已变化"));
  expect(h.result.current.document).toEqual(active); expect(h.result.current.edits).toEqual([]);
  h.unmount();
});

it("changes a Key without retaining old catalog evidence, preserving unrelated account and route ownership", () => {
  const active = routeDocument([{ id: "renamed", preset: "qwen-official-api", base_url: base, supported_models: ["fetched", "manual", "other"],
    credentials: [{ id: "a", supported_models: ["fetched", "manual"], model_selection: { fetched: ["fetched"], manual: ["manual"] } },
      { id: "b", supported_models: ["fetched", "other"], model_selection: { fetched: ["fetched"], manual: ["other"] } }] }]);
  active.model_routes = [{ pattern: "fetched", provider_ids: ["renamed"], priority: 42 }, { pattern: "manual", provider_ids: ["renamed"], enabled: false }];
  const h = setup(active); act(() => h.result.current.setMode("edit"));
  const previous = credentialDialogValueFromDocument(active, "renamed", "a")!;
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...previous, apiKeyOperation: "replace", apiKeyValue: "synthetic-new-key" }));
  const provider = h.result.current.document.providers[0] as Record<string, unknown>;
  expect((provider.credentials as Record<string, unknown>[])[0]).toMatchObject({ supported_models: ["manual"], model_selection: { fetched: [], manual: ["manual"] } });
  expect(provider.supported_models).toEqual(expect.arrayContaining(["manual", "fetched", "other"]));
  expect(h.result.current.document.model_routes).toEqual(active.model_routes);
  h.unmount();
});
