// Staging uses stable identities, preserves pending Keep, and validates again at commit.
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import type { CredentialDialogValue } from "./CredentialDialog";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

const base = "https://agents.lumalabs.ai/v1";
const value = (apiKeyOperation: CredentialDialogValue["apiKeyOperation"]): CredentialDialogValue => ({
  providerId: "renamed", credentialId: "exact", accountName: "", enabled: true, baseUrl: "",
  supportedModelsText: "uni-1", manualModelsText: "uni-1", fetchedModelsText: "",
  apiKeyOperation, apiKeyValue: apiKeyOperation === "replace" ? "luma-api-synthetic-staged" : "",
});
function setup(providerFields: Record<string, unknown> = {}) {
  const active = routeDocument([{ id: "renamed", preset: "lumalabs-api", base_url: base, explicit_credentials_only: true, credentials: [], ...providerFields }]);
  const error = vi.fn();
  const hook = renderHook(() => {
    const [document, setDocument] = useState(active), [mode, setMode] = useState<"add" | "edit">("add");
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
it("stages an audience-bound API Key without secrets, OAuth or discovery in the public document", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
  expect(h.error).toHaveBeenLastCalledWith(null);
  expect(h.result.current.edits).toEqual([expect.objectContaining({ lumaAgentsBinding: { product: "lumalabs-api", baseUrl: base, authMode: "bearer" } })]);
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "luma-api-synthetic-staged" }]);
  expect(JSON.stringify(h.result.current.document)).not.toMatch(/luma-api-synthetic|discovery_job|refresh_token/); h.unmount();
});
it.each([null, { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" }])("accepts real management null optional fields with flow %j", (call_flow) => {
  const h = setup({ adapter: null, protocol_profile: null, request_client_profile: null, call_flow });
  act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
  expect(h.error).toHaveBeenLastCalledWith(null); expect(h.patches()).toHaveLength(1); h.unmount();
});
it.each([{ adapter: "openai_compatible" }, { protocol_profile: "lumalabs" }, { protocol_family: "openai_chat" },
  { headers: { Cookie: "private-web" } }, { extra_body: { realmId: "web" } }, { execution_mode: "browser_backed" },
  { call_flow: { special_adapter: "none" } }, { call_flow: { fingerprint_adapter: "session" } }])("rejects a foreign provider surface: %j", (patch) => {
  const h = setup(patch); act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
  expect(h.error).not.toHaveBeenLastCalledWith(null); expect(h.result.current.document).toEqual(h.active); expect(h.result.current.edits).toEqual([]); h.unmount();
});
it("retains the staged Key across metadata Keep and replaces it with a normal Clear", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
  const staged = structuredClone(h.result.current.edits); act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...value("keep"), accountName: "Renamed", enabled: false }));
  expect(h.result.current.edits).toEqual(staged); expect(h.patches()).toHaveLength(1);
  act(() => h.result.current.editor.applyCredentialDialogValue(value("clear")));
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "clear" }]); h.unmount();
});
it("rejects changed product or audience after staging, and never exposes the material in preview", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
  for (const patch of [{ preset: "lumalabs" }, { base_url: "https://untrusted.invalid" }, { auth_mode: "session" },
    { protocol_profile: "lumalabs" }, { call_flow: { call_adapter: "openai" } }]) {
    const changed = structuredClone(h.result.current.document); Object.assign(changed.providers[0] as object, patch);
    expect(() => h.patches(changed)).toThrow(); expect(h.patches(changed, false)).toEqual([]);
  }
  h.unmount();
});
it("rebases by stable IDs and refuses a replacement with no captured binding", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(value("replace")));
  const reordered = structuredClone(h.result.current.document); reordered.providers.unshift({ id: "other" });
  ((reordered.providers[1] as Record<string, unknown>).credentials as unknown[]).unshift({ id: "sibling" });
  expect(h.patches(reordered)).toEqual([{ path: "/providers/1/credentials/1/api_key", operation: "replace", value: "luma-api-synthetic-staged" }]);
  expect(() => buildCredentialSecretPatches({ activeDocument: h.active, draftDocument: h.result.current.document, activeSecrets: [],
    credentialSecretEdits: [{ providerId: "renamed", credentialId: "exact", field: "api_key", operation: "replace", value: "luma-api-unbound" }] })).toThrow(); h.unmount();
});
it.each([{ baseUrl: "https://untrusted.invalid" }, { manualModelsText: "uni-unknown" }, { fetchedModelsText: "uni-1" },
  { apiKeyValue: "wos-session=web" }, { apiKeyValue: "luma-api-two words" }, { apiKeyValue: "luma-api-" },
  { apiKeyValue: `luma-api-${"x".repeat(4096)}` }])("rejects invalid account input before staging: %j", (patch) => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue({ ...value("replace"), ...patch }));
  expect(h.error).not.toHaveBeenLastCalledWith(null); expect(h.result.current.document).toEqual(h.active); expect(h.result.current.edits).toEqual([]); h.unmount();
});
