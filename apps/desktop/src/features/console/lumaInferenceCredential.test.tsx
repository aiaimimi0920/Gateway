import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { CredentialDialogValue } from "./credentialDialogTypes";
import { normalizeLumaInferenceCredential } from "./lumaInferenceCredential";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";

const provider = { id: "llm", preset: "lumalabs-inference-api", base_url: "https://inference.lumalabs.ai/v1",
  explicit_credentials_only: true, credentials: [{ id: "stable" }] };
const value: CredentialDialogValue = { providerId: "llm", credentialId: "stable", accountName: "Inference",
  enabled: true, baseUrl: "", supportedModelsText: "glm-5.2", apiKeyOperation: "replace", apiKeyValue: "inference-synthetic" };

it("rejects media keys and foreign targets without inventing key prefixes or model discovery", () => {
  expect(normalizeLumaInferenceCredential(provider, value)).toMatchObject({ apiKeyValue: "inference-synthetic", baseUrl: "" });
  for (const patch of [{ apiKeyValue: "luma-api-media" }, { apiKeyValue: "has\nnewline" },
    { baseUrl: "https://agents.lumalabs.ai/v1" }]) {
    expect(() => normalizeLumaInferenceCredential(provider, { ...value, ...patch })).toThrow(/Luma/);
  }
});

it("saves, reopens, keeps staged replacement and explicitly clears through the normal editor", () => {
  let draft = { providers: [provider], model_routes: [], aliases: {} };
  let secrets: CredentialSecretEdit[] = [];
  const replaceEditorDocument = vi.fn(next => { draft = next; });
  const setError = vi.fn();
  const { result, rerender } = renderHook(() => useCredentialDialogEditor({ editorText: JSON.stringify(draft),
    credentialDialogState: { mode: "edit", initialValue: null }, setCredentialDialogState: vi.fn(),
    setCredentialSecretEdits: update => { secrets = typeof update === "function" ? update(secrets) : update; },
    setError, replaceEditorDocument, t: zh => zh }));
  act(() => result.current.applyCredentialDialogValue(value));
  expect(secrets).toEqual([{ providerId: "llm", credentialId: "stable", field: "api_key", operation: "replace", value: "inference-synthetic",
    lumaInferenceBinding: { product: "lumalabs-inference-api", baseUrl: provider.base_url } }]);
  expect(JSON.stringify(draft)).not.toContain("inference-synthetic");
  expect(JSON.stringify(draft)).not.toContain("discovery_job");
  rerender();
  const reopened = credentialDialogValueFromDocument(draft, "llm", "stable")!;
  act(() => result.current.applyCredentialDialogValue({ ...reopened, accountName: "Renamed", apiKeyOperation: "keep" }));
  expect(secrets).toHaveLength(1);
  expect(secrets[0]).toMatchObject({ operation: "replace", value: "inference-synthetic" });
  const changed = structuredClone(draft);
  changed.providers[0].base_url = "https://foreign.invalid/v1";
  expect(() => buildCredentialSecretPatches({ activeDocument: draft, draftDocument: changed,
    activeSecrets: [], credentialSecretEdits: secrets })).toThrow(/Luma/);
  rerender();
  act(() => result.current.applyCredentialDialogValue({ ...reopened, apiKeyOperation: "clear" }));
  expect(secrets).toEqual([{ providerId: "llm", credentialId: "stable", field: "api_key", operation: "clear" }]);
  expect(draft.providers[0].credentials[0].id).toBe("stable");
  expect(setError).toHaveBeenLastCalledWith(null);
});
