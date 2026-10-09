import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useAccountDiscovery } from "./useAccountDiscovery";

const document = { providers: [{ id: "p", adapter: "openai_compatible", base_url: "https://fixture.test",
  credentials: [{ id: "a", supported_models: ["old"] }] }], model_routes: [], aliases: {} };
const result = { revision: "r1", discovery: { source_url: "https://fixture.test", api_base: "https://fixture.test/v1",
  protocol: "chat_completions", models: ["new"], verified_models: ["new"], checked_at: "now", binding: "fixture" } };

function options(request = vi.fn().mockResolvedValue(result)) {
  return { client: { request }, managementToken: "manager", secretGrant: "grant", editorText: JSON.stringify(document),
    revision: "r1", draftDirty: false, replaceEditorDocument: vi.fn(), requestSecretAccess: vi.fn(), setError: vi.fn() };
}

it("refresh uses stored credential identity and replaces only the successful draft", async () => {
  const opts = options(); const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("p", "a"));
  expect(opts.client.request).toHaveBeenCalledWith(expect.any(String), expect.anything(), expect.objectContaining({ body: { credentialId: "a" } }));
  expect(opts.replaceEditorDocument).toHaveBeenCalledWith(expect.objectContaining({
    providers: [expect.objectContaining({ credentials: [expect.objectContaining({ discovery: result.discovery, supported_models: ["new"] })] })],
  }), true);
});

it("failed refresh leaves the old document intact", async () => {
  const opts = options(vi.fn().mockRejectedValue(new Error("offline")));
  const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("p", "a"));
  expect(opts.replaceEditorDocument).not.toHaveBeenCalled();
  expect(opts.setError).toHaveBeenCalledWith("offline");
});

it("rejects a discovery response from a different revision", async () => {
  const opts = options(vi.fn().mockResolvedValue({ ...result, revision: "r2" }));
  const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("p", "a"));
  expect(opts.replaceEditorDocument).not.toHaveBeenCalled();
  expect(opts.setError).toHaveBeenCalledWith(expect.stringContaining("配置或登录状态已变化"));
});

it("blocks refresh while an unsaved edit exists", async () => {
  const opts = { ...options(), draftDirty: true };
  const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("p", "a"));
  expect(opts.client.request).not.toHaveBeenCalled();
});
