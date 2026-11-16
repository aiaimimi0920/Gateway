import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useAccountDiscovery } from "./useAccountDiscovery";

const base = "https://token.sensenova.cn/v1";
const catalog = { sourceUrl: `${base}/models`, checkedAt: "2026-11-10T00:00:00Z", models: ["sensenova-live-text"] };
const doc = { providers: [{ id: "renamed", preset: "sensenova-token-plan", base_url: base, credentials: [
  { id: "stable", supported_models: ["manual"] },
] }], model_routes: [], aliases: {} };
function options(request = vi.fn().mockResolvedValue({ revision: "r1", catalog })) {
  return { client: { request }, managementToken: "manager", secretGrant: "grant", editorText: JSON.stringify(doc), revision: "r1",
    draftDirty: false, replaceEditorDocument: vi.fn(), requestSecretAccess: vi.fn(), setError: vi.fn() };
}

it("routes an existing SenseNova account to the catalog handler and stages normal saving", async () => {
  const opts = options(); const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("renamed", "stable"));
  expect(opts.client.request).toHaveBeenCalledWith("/v1/internal/gateway/console/sensenova-model-catalog", expect.anything(),
    expect.objectContaining({ body: { credentialId: "stable" }, secretGrant: "grant" }));
  expect(opts.replaceEditorDocument).toHaveBeenCalledWith(expect.objectContaining({ providers: [expect.objectContaining({ credentials: [
    expect.objectContaining({ id: "stable", model_catalog: catalog, supported_models: ["manual", "sensenova-live-text"] }),
  ] })] }), true);
  hook.unmount();
});

it("reads a newly supplied Key without writing it into the editor document", async () => {
  const opts = options(); const hook = renderHook(() => useAccountDiscovery(opts));
  const body = { providerId: "renamed", apiKey: "cpk-synthetic" };
  await act(async () => expect(await hook.result.current.discoverProviderModels(body)).toEqual(catalog));
  expect(opts.client.request).toHaveBeenCalledWith("/v1/internal/gateway/console/sensenova-model-catalog", expect.anything(),
    expect.objectContaining({ body, secretGrant: "grant" }));
  expect(opts.replaceEditorDocument).not.toHaveBeenCalled(); hook.unmount();
});

it.each(["revision", "editorText", "secretGrant", "managementToken"])("discards a late SenseNova catalog after %s changes", async (field) => {
  let resolve!: (value: unknown) => void;
  const opts = options(vi.fn(() => new Promise((done) => { resolve = done; })));
  const hook = renderHook((value) => useAccountDiscovery(value), { initialProps: opts });
  let pending!: Promise<void>;
  act(() => { pending = hook.result.current.refreshAccountDiscovery("renamed", "stable"); });
  hook.rerender({ ...opts, [field]: field === "editorText" ? JSON.stringify({ ...doc, aliases: { changed: "manual" } }) : "changed" });
  await act(async () => { resolve({ revision: "r1", catalog }); await pending; });
  expect(opts.replaceEditorDocument).not.toHaveBeenCalled(); hook.unmount();
});
