import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useAccountDiscovery } from "./useAccountDiscovery";

const document = { providers: [{ id: "chatgpt", preset: "chatgpt-codex-oauth-official-api",
  supported_models: ["declared"], credentials: [{ id: "account", supported_models: ["declared"] }] }],
  model_routes: [{ pattern: "new", provider_ids: ["other"], priority: 7, enabled: false }], aliases: {} };
const result = { revision: "r1", catalog: { clientVersion: "0.155.1", checkedAt: "now", models: [
  { slug: "new", visibility: "list", supported_in_api: true },
  { slug: "hidden", visibility: "hide", supported_in_api: true },
  { slug: "not-api", visibility: "list", supported_in_api: false },
] } };

function options(request = vi.fn().mockResolvedValue(result)) {
  return { client: { request }, managementToken: "manager", secretGrant: "grant", editorText: JSON.stringify(document),
    revision: "r1", draftDirty: false, replaceEditorDocument: vi.fn(), requestSecretAccess: vi.fn(), setError: vi.fn() };
}

it("refreshes stored ChatGPT catalog without pretending catalog entries passed generation", async () => {
  const opts = options(); const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("chatgpt", "account"));
  expect(opts.client.request).toHaveBeenCalledWith("/v1/internal/gateway/console/chatgpt-model-catalog", expect.anything(),
    expect.objectContaining({ body: { credentialId: "account" } }));
  const saved = opts.replaceEditorDocument.mock.calls[0][0];
  expect(saved.providers[0].credentials[0]).toEqual({ id: "account", supported_models: ["declared", "new"] });
  expect(saved.providers[0].supported_models).toEqual(["declared", "new"]);
  expect(saved.model_routes[0]).toEqual({ pattern: "new", provider_ids: ["other", "chatgpt"], priority: 7, enabled: false });
  expect(JSON.stringify(saved)).not.toContain("verified_models");
});

it("keeps ChatGPT models unchanged when the catalog fails or has a stale revision", async () => {
  for (const request of [vi.fn().mockRejectedValue(new Error("offline")), vi.fn().mockResolvedValue({ ...result, revision: "r2" })]) {
    const opts = options(request); const hook = renderHook(() => useAccountDiscovery(opts));
    await act(() => hook.result.current.refreshAccountDiscovery("chatgpt", "account"));
    expect(opts.replaceEditorDocument).not.toHaveBeenCalled();
    expect(opts.setError).toHaveBeenCalled();
    hook.unmount();
  }
});
