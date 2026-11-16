// Stable-key staging and the real account editor must preserve ownership and ordinary save semantics.
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { freebuffBusinessFields, savedFreebuffBusiness, stageFreebuffCatalog } from "./freebuffBusiness";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import type { FreebuffCatalog } from "./freebuffCatalog";
const target = { origin: "https://codebuff.com", client: "cli" } as const;
const document = { providers: [{ id: "pool", preset: "freebuff", base_url: "https://freebuff.com", explicit_credentials_only: true,
  supported_models: ["old", "manual", "sibling"], credentials: [
    { id: "account", api_key: null, enabled: false, supported_models: ["manual", "old"],
      model_selection: { fetched: ["old"], manual: ["manual"] }, extra_body: { custom: { retained: true } },
      freebuff_session: { fingerprint_id: null, fingerprint_hash: null } },
    { id: "other", supported_models: ["sibling"] },
  ] }], model_routes: [{ pattern: "old", provider_ids: ["pool", "elsewhere"], priority: 42, enabled: true },
    { pattern: "new", provider_ids: ["elsewhere"], priority: 99, enabled: false }], aliases: { alias: "manual" } };
const row = (key: string, access: "open" | "locked" = "open") => ({ key, access, displayName: key, tagline: "", warning: null,
  badges: [], multimodal: false, premium: false, dataUse: "service" as const });
const catalog: FreebuffCatalog = { providerId: "pool", credentialId: "account", revision: "r1",
  businessOrigin: target.origin, client: target.client, models: [row("new"), row("locked", "locked")] };
const initial = () => credentialDialogValueFromDocument(document, "pool", "account")!;

it("stages stable keys without changing token operation, manual ownership or the source document", () => {
  const value = initial(), before = structuredClone(value);
  const staged = stageFreebuffCatalog(value, catalog, ["new", "new"]);
  expect(staged).toMatchObject({ freebuffBusiness: target, manualModelsText: "manual", fetchedModelsText: "new", apiKeyOperation: "keep", apiKeyValue: "" });
  expect(value).toEqual(before); expect(JSON.stringify(staged)).not.toMatch(/handle|fetchId|fbm1\./);
});
it.each([[], ["locked"], ["foreign"], ["*"], ["new", "foreign"]].map((keys) => ({ keys })))("rejects an unavailable or invalid selection: $keys", ({ keys }) => {
  expect(() => stageFreebuffCatalog(initial(), catalog, keys)).toThrow();
});
it.each([{ providerId: "foreign" }, { credentialId: "foreign" }, { apiKeyOperation: "replace" as const }])("refuses to borrow directory selection for another material identity: %j", (patch) => {
  expect(() => stageFreebuffCatalog({ ...initial(), ...patch }, catalog, ["new"])).toThrow();
});
it("uses the normal editor to reconcile models/routes and retain all unrelated account data", () => {
  const replaceEditorDocument = vi.fn(), setError = vi.fn(), setCredentialSecretEdits = vi.fn();
  const h = renderHook(() => useCredentialDialogEditor({ editorText: JSON.stringify(document),
    credentialDialogState: { mode: "edit", initialValue: initial() }, setCredentialDialogState: vi.fn(),
    setCredentialSecretEdits, setError, replaceEditorDocument, t: (zh) => zh }));
  act(() => h.result.current.applyCredentialDialogValue(stageFreebuffCatalog(initial(), catalog, ["new"])));
  expect(setError).toHaveBeenLastCalledWith(null); const saved = JSON.parse(JSON.stringify(replaceEditorDocument.mock.lastCall![0]));
  expect(saved.providers[0].base_url).toBe("https://freebuff.com");
  expect(saved.providers[0].credentials[0]).toMatchObject({ id: "account", api_key: null, enabled: false,
    extra_body: { custom: { retained: true }, freebuffBusiness: target }, freebuff_session: document.providers[0].credentials[0].freebuff_session,
    model_selection: { fetched: ["new"], manual: ["manual"] }, supported_models: ["manual", "new"] });
  expect(saved.providers[0].credentials[1]).toEqual(document.providers[0].credentials[1]);
  expect(saved.providers[0].supported_models).toEqual(["sibling", "manual", "new"]);
  expect(saved.model_routes).toContainEqual({ pattern: "old", provider_ids: ["elsewhere"], priority: 42, enabled: true });
  expect(saved.model_routes).toContainEqual({ pattern: "new", provider_ids: ["elsewhere", "pool"], priority: 99, enabled: false });
  expect(saved.aliases).toEqual(document.aliases); expect(savedFreebuffBusiness(saved.providers[0], "account")).toEqual(target);
  expect(savedFreebuffBusiness(saved.providers[0], "other")).toBeUndefined();
  const reopened = credentialDialogValueFromDocument(saved, "pool", "account")!;
  expect(reopened).toMatchObject({ fetchedModelsText: "new", manualModelsText: "manual", apiKeyOperation: "keep" });
  expect(setCredentialSecretEdits).not.toHaveBeenCalled(); h.unmount();
});
it("only recognizes an exact account-owned target and cannot move it to a new or foreign product", () => {
  const staged = stageFreebuffCatalog(initial(), catalog, ["new"]);
  expect(freebuffBusinessFields(document.providers[0], initial())).toEqual({});
  for (const provider of [undefined, { ...document.providers[0], preset: "other" }, { ...document.providers[0], credentials: [] }]) {
    expect(() => freebuffBusinessFields(provider, staged)).toThrow();
  }
  expect(savedFreebuffBusiness({ ...document.providers[0], extra_body: { freebuffBusiness: target } }, "other")).toBeUndefined();
  expect(savedFreebuffBusiness({ ...document.providers[0], credentials: [{ id: "account", extra_body: { freebuffBusiness: { ...target, handle: "fbm1.private" } } }] }, "account")).toBeUndefined();
});
