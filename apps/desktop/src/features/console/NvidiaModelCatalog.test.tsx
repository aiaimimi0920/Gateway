import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { useAccountDiscovery } from "./useAccountDiscovery";
import { applyNvidiaCatalog, type NvidiaCatalog } from "./nvidiaModelCatalog";

const catalog: NvidiaCatalog = { sourceUrl: "https://integrate.api.nvidia.com/v1/models", checkedAt: "2026-10-26T04:00:00Z", models: ["openai/gpt-oss-20b", "new-model"] };
const document: ConsoleRouteDocument = { providers: [{ id: "nvidia", preset: "nvidia-openai", supported_models: ["manual", "old"],
  credentials: [{ id: "one", api_key: null, supported_models: ["manual", "old"], model_catalog: { ...catalog, models: ["old"] } },
    { id: "two", api_key: null, supported_models: ["other"] }] }], aliases: { preserved: "manual" },
  model_routes: [{ pattern: "old", provider_ids: ["nvidia", "other"], priority: 42 },
    { pattern: "new-model", provider_ids: ["other"], enabled: false, priority: 99 }] };

it("refreshes catalog-owned models and route membership without changing manual policy or other accounts", () => {
  const before = structuredClone(document);
  const next = applyNvidiaCatalog(document, "nvidia", "one", catalog);
  expect(document).toEqual(before);
  expect(next.providers[0]).toMatchObject({ supported_models: ["manual", ...catalog.models, "other"],
    credentials: [{ id: "one", supported_models: ["manual", ...catalog.models], model_catalog: catalog },
      { id: "two", api_key: null, supported_models: ["other"] }] });
  expect(next.model_routes).toContainEqual({ pattern: "old", provider_ids: ["other"], priority: 42 });
  expect(next.model_routes).toContainEqual({ pattern: "new-model", provider_ids: ["other", "nvidia"], enabled: false, priority: 99 });
  expect(next.aliases).toEqual(before.aliases);
  expect(JSON.stringify(next)).not.toContain("verified_models");
});

it("uses the stored NVIDIA catalog endpoint rather than generative protocol discovery", async () => {
  const opts = { client: { request: vi.fn().mockResolvedValue({ revision: "r1", catalog }) } as unknown as GatewayApiClient,
    managementToken: "fixture", secretGrant: "grant", editorText: JSON.stringify(document), revision: "r1", draftDirty: false,
    replaceEditorDocument: vi.fn(), requestSecretAccess: vi.fn(), setError: vi.fn() };
  const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("nvidia", "one"));
  expect(opts.client.request).toHaveBeenCalledWith("/v1/internal/gateway/console/nvidia-model-catalog", expect.anything(),
    expect.objectContaining({ body: { credentialId: "one" } }));
  expect(opts.replaceEditorDocument).toHaveBeenCalledWith(applyNvidiaCatalog(document, "nvidia", "one", catalog), true);
});

it("is idempotent when a previously declared default also occurs later in the live catalog", () => {
  const before = { providers: [{ id: "nvidia", preset: "nvidia-openai", supported_models: ["openai/gpt-oss-20b"],
    credentials: [{ id: "one", supported_models: ["openai/gpt-oss-20b"] }] }], model_routes: [], aliases: {} };
  const input = { ...catalog, models: ["new-model", "openai/gpt-oss-20b"] };
  const once = applyNvidiaCatalog(before, "nvidia", "one", input);
  expect(applyNvidiaCatalog(once, "nvidia", "one", input)).toEqual(once);
});

it("preserves the last-good draft on catalog failures or stale revisions", async () => {
  for (const request of [vi.fn().mockRejectedValue(new Error("offline")), vi.fn().mockResolvedValue({ revision: "r2", catalog })]) {
    const opts = { client: { request } as unknown as GatewayApiClient, managementToken: "fixture", secretGrant: "grant",
      editorText: JSON.stringify(document), revision: "r1", draftDirty: false,
      replaceEditorDocument: vi.fn(), requestSecretAccess: vi.fn(), setError: vi.fn() };
    const hook = renderHook(() => useAccountDiscovery(opts));
    await act(() => hook.result.current.refreshAccountDiscovery("nvidia", "one"));
    expect(opts.replaceEditorDocument).not.toHaveBeenCalled();
    expect(opts.setError).toHaveBeenCalledWith(expect.any(String));
  }
});
