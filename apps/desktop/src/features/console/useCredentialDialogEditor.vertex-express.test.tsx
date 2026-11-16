// Pending material is bound by stable IDs and product, and never copied into a public document.
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import type { CredentialDialogValue } from "./CredentialDialog";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

const base = "https://aiplatform.googleapis.com/v1/publishers/google";
const value = (apiKeyOperation: CredentialDialogValue["apiKeyOperation"]): CredentialDialogValue => ({
  providerId: "renamed", credentialId: "exact", accountName: "", enabled: true, baseUrl: "",
  supportedModelsText: "gemini-contract", manualModelsText: "gemini-contract", fetchedModelsText: "",
  apiKeyOperation, apiKeyValue: apiKeyOperation === "replace" ? "synthetic-staged-key" : "",
});
function setup(providerFields: Record<string, unknown> = {}) {
  const active = routeDocument([{ id: "renamed", preset: "vertex-express-api", base_url: base, explicit_credentials_only: true, credentials: [], ...providerFields }]);
  const error = vi.fn();
  const hook = renderHook(() => {
    const [document, setDocument] = useState(active);
    const [mode, setMode] = useState<"add" | "edit">("add");
    const [edits, setEdits] = useState<CredentialSecretEdit[]>([]);
    const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode, initialValue: null },
      setCredentialDialogState: vi.fn(), setCredentialSecretEdits: setEdits, setError: error, replaceEditorDocument: setDocument, t: (zh) => zh });
    return { editor, document, edits, setMode };
  });
  const patches = (document: ConsoleRouteDocument = hook.result.current.document, strictSessionBindings = true) => buildCredentialSecretPatches({
    activeDocument: active, draftDocument: document, activeSecrets: [], credentialSecretEdits: hook.result.current.edits, strictSessionBindings,
  });
  return { ...hook, active, error, patches };
}
it.each([{ adapter: "" }, { adapter: "openai_compatible" }, { protocol_profile: false },
  { request_client_profile: "browser" }, { call_flow: false }])("still rejects a non-null invalid override: %j", (patch) => {
  const h = setup(patch); act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
  expect(h.error).not.toHaveBeenLastCalledWith(null); expect(h.result.current.document).toEqual(h.active);
  expect(h.result.current.edits).toEqual([]); h.unmount();
});
it("stages an audience-bound Express Key without secret, refresh or discovery in the public draft", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
  expect(h.error).toHaveBeenLastCalledWith(null);
  expect(h.result.current.edits).toEqual([expect.objectContaining({ vertexExpressBinding: {
    product: "vertex-express-api", baseUrl: base, authMode: "vertex_express_api_key" } })]);
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-staged-key" }]);
  expect(JSON.stringify(h.result.current.document)).not.toMatch(/synthetic-|discovery_job|refresh_token/); h.unmount();
});
it.each([null, { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" }])(
  "accepts unset optional fields serialized as null by the real management API, with flow %j", (call_flow) => {
    const h = setup({ adapter: null, protocol_profile: null, request_client_profile: null, call_flow });
    act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
    expect(h.error).toHaveBeenLastCalledWith(null);
    expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-staged-key" }]);
    h.unmount();
  });
it("retains the staged replacement across metadata Keep and replaces it on clear", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
  const staged = structuredClone(h.result.current.edits);
  act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...value("keep"), accountName: "Renamed" }));
  expect(h.result.current.edits).toEqual(staged); expect(h.patches()).toHaveLength(1);
  act(() => h.result.current.editor.applyCredentialDialogValue(value("clear")));
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "clear" }]); h.unmount();
});
it("rejects product, target, flow or mode changes after staging even in preview", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
  for (const patch of [{ preset: "gemini-api" }, { base_url: "https://untrusted.invalid" },
    { auth_mode: "bearer" }, { protocol_profile: "antigravity_oauth" }, { call_flow: { call_adapter: "openai" } }]) {
    const changed = structuredClone(h.result.current.document); Object.assign(changed.providers[0] as object, patch);
    expect(() => h.patches(changed)).toThrow(); expect(h.patches(changed, false)).toEqual([]);
  }
  h.unmount();
});
it("rebases the bound replacement by IDs and refuses an unbound replacement", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
  const reordered = structuredClone(h.result.current.document); reordered.providers.unshift({ id: "other" });
  ((reordered.providers[1] as Record<string, unknown>).credentials as unknown[]).unshift({ id: "sibling" });
  expect(h.patches(reordered)).toEqual([{ path: "/providers/1/credentials/1/api_key", operation: "replace", value: "synthetic-staged-key" }]);
  expect(() => buildCredentialSecretPatches({ activeDocument: h.active, draftDocument: h.result.current.document, activeSecrets: [],
    credentialSecretEdits: [{ providerId: "renamed", credentialId: "exact", field: "api_key", operation: "replace", value: "unbound" }] })).toThrow();
  h.unmount();
});
it.each([{ baseUrl: "https://untrusted.invalid" }, { manualModelsText: "models/foo" }, { fetchedModelsText: "gemini-contract" },
  { apiKeyValue: "two words" }])("refuses invalid account input before changing draft or material: %j", (patch) => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue({ ...value("replace"), ...patch }));
  expect(h.error).not.toHaveBeenLastCalledWith(null); expect(h.result.current.document).toEqual(h.active);
  expect(h.result.current.edits).toEqual([]); h.unmount();
});
