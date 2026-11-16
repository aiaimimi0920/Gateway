import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useAccountDiscovery } from "./useAccountDiscovery";

const base = "https://workspace-a.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1";
const catalog = { sourceUrl: "https://dashscope-intl.aliyuncs.com/api/v1/models", accountBaseUrl: base,
  checkedAt: "2026-11-02T00:00:00Z", models: ["fetched"] };
const doc = { providers: [{ id: "renamed", preset: "qwen-official-api", base_url: base, credentials: [
  { id: "a", base_url: base.replace("workspace-a", "workspace-b"), supported_models: ["manual"] },
] }], model_routes: [], aliases: {} };
function options(request = vi.fn().mockResolvedValue({ revision: "r1", catalog: { ...catalog, accountBaseUrl: base.replace("workspace-a", "workspace-b") } })) {
  return { client: { request }, managementToken: "manager", secretGrant: "grant", editorText: JSON.stringify(doc), revision: "r1",
    draftDirty: false, replaceEditorDocument: vi.fn(), requestSecretAccess: vi.fn(), setError: vi.fn() };
}
it("refreshes the exact override account and saves both native source and workspace binding", async () => {
  const opts = options(); const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("renamed", "a"));
  expect(opts.client.request).toHaveBeenCalledWith("/v1/internal/gateway/console/qwen-model-catalog", expect.anything(),
    expect.objectContaining({ body: { credentialId: "a" }, secretGrant: "grant" }));
  expect(opts.replaceEditorDocument).toHaveBeenCalledWith(expect.objectContaining({ providers: [expect.objectContaining({ credentials: [
    expect.objectContaining({ model_catalog: { ...catalog, accountBaseUrl: base.replace("workspace-a", "workspace-b") }, supported_models: ["manual", "fetched"] }),
  ] })] }), true);
  hook.unmount();
});
it("rejects a shared-source directory for the wrong workspace without changing any document", async () => {
  const opts = options(vi.fn().mockResolvedValue({ revision: "r1", catalog }));
  const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("renamed", "a"));
  expect(opts.replaceEditorDocument).not.toHaveBeenCalled();
  expect(opts.setError).toHaveBeenCalledWith(expect.stringContaining("账号目标不匹配")); hook.unmount();
});
it("uses a validated explicit draft target only with a newly supplied Key", async () => {
  const opts = options(vi.fn().mockResolvedValue({ revision: "r1", catalog }));
  const hook = renderHook(() => useAccountDiscovery(opts));
  const input = { providerId: "renamed", apiKey: "synthetic-key", accountBaseUrl: base };
  await act(async () => expect(await hook.result.current.discoverProviderModels(input)).toEqual(catalog));
  expect(opts.client.request).toHaveBeenCalledWith(expect.stringContaining("qwen-model-catalog"), expect.anything(), expect.objectContaining({ body: input }));
  await act(async () => expect(hook.result.current.discoverProviderModels({ ...input, accountBaseUrl: "https://evil.invalid" })).rejects.toThrow());
  expect(opts.client.request).toHaveBeenCalledTimes(1); hook.unmount();
});
it("does not use an active stored Key to fetch evidence for unsaved staged edits", async () => {
  const opts = { ...options(), draftDirty: true }; const hook = renderHook(() => useAccountDiscovery(opts));
  await act(async () => expect(hook.result.current.discoverProviderModels({ credentialId: "a" })).rejects.toThrow("请先保存"));
  expect(opts.client.request).not.toHaveBeenCalled(); hook.unmount();
});
it.each(["revision", "editorText", "secretGrant", "managementToken"])("fences a late directory after %s changes", async (field) => {
  let resolve!: (value: unknown) => void;
  const opts = options(vi.fn(() => new Promise((done) => { resolve = done; })));
  const hook = renderHook((value) => useAccountDiscovery(value), { initialProps: opts });
  let pending!: Promise<void>;
  act(() => { pending = hook.result.current.refreshAccountDiscovery("renamed", "a"); });
  hook.rerender({ ...opts, [field]: field === "editorText" ? JSON.stringify({ ...doc, aliases: { changed: "fetched" } }) : "changed" });
  await act(async () => { resolve({ revision: "r1", catalog }); await pending; });
  expect(opts.replaceEditorDocument).not.toHaveBeenCalled(); hook.unmount();
});
