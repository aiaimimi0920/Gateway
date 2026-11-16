import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ConsoleRouteConfigResponse, ConsoleRouteDocument } from "../../api/contracts";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";
import { qwenMaterialTarget } from "./qwenMaterialBinding";
import { useConsoleRouteDraft } from "./useConsoleRouteDraft";

const base = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const document = (): ConsoleRouteDocument => routeDocument([{ id: "renamed", preset: "qwen-official-api", base_url: base,
  credentials: [{ id: "a" }] }]);
const staged = (active: ConsoleRouteDocument): CredentialSecretEdit => ({ providerId: "renamed", credentialId: "a", field: "api_key",
  operation: "replace", value: "synthetic-staged-qwen", qwenBinding: qwenMaterialTarget(active, "renamed", "a") });
const patches = (draftDocument: ConsoleRouteDocument, edit: CredentialSecretEdit, strictSessionBindings = true) => buildCredentialSecretPatches({
  activeDocument: document(), draftDocument, activeSecrets: [], credentialSecretEdits: [edit], strictSessionBindings });

describe("ordinary Qwen staged Key", () => {
  it.each(["parent", "override", "auth", "preset"])("rejects %s target drift at final patch construction and omits unsafe previews", (kind) => {
    const active = document(); const edit = staged(active); const draft = structuredClone(active);
    const provider = draft.providers[0] as Record<string, unknown>;
    const account = (provider.credentials as Record<string, unknown>[])[0];
    if (kind === "parent") provider.base_url = base.replace("cn-beijing", "eu-central-1");
    if (kind === "override") account.base_url = base.replace("workspace-a", "workspace-b");
    if (kind === "auth") account.auth_mode = "x-api-key";
    if (kind === "preset") provider.preset = "qwen-coding-plan";
    expect(() => patches(draft, edit)).toThrow();
    expect(patches(draft, edit, false)).toEqual([]);
    expect(edit).toEqual(staged(active));
  });
  it("requires a binding for every ordinary Qwen editor replacement, without changing generic API semantics", () => {
    const active = document(); const edit = staged(active); delete edit.qwenBinding;
    expect(() => patches(active, edit)).toThrow("材料未绑定目标");
    expect(patches(active, edit, false)).toEqual([]);
    (active.providers[0] as Record<string, unknown>).preset = "openai";
    expect(patches(active, edit)).toContainEqual({ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-staged-qwen" });
  });
  it("uses stable identities after reordering and ignores representation-only inheritance changes", () => {
    const active = document(); const edit = staged(active); const draft = structuredClone(active);
    const provider = draft.providers[0] as Record<string, unknown>;
    (provider.credentials as Record<string, unknown>[]).unshift({ id: "other", base_url: base.replace("workspace-a", "workspace-b") });
    (provider.credentials as Record<string, unknown>[])[1].base_url = `${base}/`;
    provider.base_url = base.replace("cn-beijing", "eu-central-1");
    draft.providers.unshift({ id: "different-provider", preset: "openai" });
    expect(patches(draft, edit)).toEqual([{ path: "/providers/1/credentials/1/api_key", operation: "replace", value: "synthetic-staged-qwen" }]);
    expect(JSON.stringify(edit.qwenBinding)).not.toContain("synthetic");
  });
  it("allows public metadata changes without rebinding a secret", () => {
    const active = document(); const edit = staged(active);
    const provider = active.providers[0] as Record<string, unknown>;
    provider.label = "New label"; provider.supported_models = ["manual"];
    Object.assign((provider.credentials as Record<string, unknown>[])[0], { account_name: "Renamed", enabled: false });
    expect(patches(active, edit)).toHaveLength(1);
  });
  it("validates the effective endpoint, product and auth rather than an editable ID suffix", () => {
    const active = document(); const provider = active.providers[0] as Record<string, unknown>;
    const account = (provider.credentials as Record<string, unknown>[])[0];
    provider.base_url = ""; account.base_url = base; account.auth_mode = "bearer";
    expect(qwenMaterialTarget(active, "renamed", "a")).toEqual({ product: "qwen-official-api", baseUrl: base, authMode: "bearer" });
    account.base_url = "https://coding.dashscope.aliyuncs.com/v1";
    expect(() => qwenMaterialTarget(active, "renamed", "a")).toThrow();
  });
});

it("the actual commit owner rechecks its final document even with an explicit secret patch override", () => {
  const active = document();
  const routeConfig: ConsoleRouteConfigResponse = { routeConfig: { revision: { id: "fixture-r1", sequence: 1 }, source: "local", document: active,
    diagnostics: { diagnostics: [] }, requiresRepair: false, mutationSupported: true, secrets: [] } };
  const options = { routeConfig, hasSecretAccess: true, t: (zh: string) => zh, invalidateCredentialProbes: vi.fn(),
    accountGroupDraftRowsFromDocument: () => [], onRouteConfigHydrated: vi.fn(), setError: vi.fn(), setSecretDialogOpen: vi.fn() };
  const { result } = renderHook(() => useConsoleRouteDraft(options));
  act(() => result.current.setCredentialSecretEdits([staged(active)]));
  expect(result.current.parseDraft().secretPatches).toHaveLength(1);
  const changed = structuredClone(active); (changed.providers[0] as Record<string, unknown>).base_url = base.replace("cn-beijing", "eu-central-1");
  act(() => result.current.replaceEditorDocument(changed));
  expect(JSON.stringify(result.current.secretPatches)).not.toContain("synthetic-staged-qwen");
  expect(() => result.current.parseDraft()).toThrow("目标已变化");
  expect(() => result.current.buildCommitRequest(changed, undefined, [])).toThrow("目标已变化");
  act(() => result.current.replaceEditorDocument(active));
  expect(result.current.parseDraft().secretPatches).toContainEqual({ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-staged-qwen" });
});
