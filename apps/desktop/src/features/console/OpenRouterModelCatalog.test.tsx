import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { useAccountDiscovery } from "./useAccountDiscovery";

const catalog = { sourceUrl: "https://openrouter.ai/api/v1/models", checkedAt: "2026-10-26T04:00:00Z", models: ["vendor/one", "vendor/two"] };
const document = { providers: [{ id: "or", preset: "openrouter-openai", base_url: "https://openrouter.ai/api/v1",
  credentials: [{ id: "one", supported_models: ["openrouter/free"] }] }], model_routes: [], aliases: {} };

it("refreshes the dedicated OpenRouter template through catalog-only GET metadata, never generation discovery", async () => {
  const opts = { client: { request: vi.fn().mockResolvedValue({ revision: "r1", catalog }) } as unknown as GatewayApiClient,
    managementToken: "fixture", secretGrant: "grant", editorText: JSON.stringify(document), revision: "r1", draftDirty: false,
    replaceEditorDocument: vi.fn(), requestSecretAccess: vi.fn(), setError: vi.fn() };
  const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("or", "one"));
  expect(opts.client.request).toHaveBeenCalledExactlyOnceWith("/v1/internal/gateway/console/openrouter-model-catalog", expect.anything(),
    expect.objectContaining({ body: { credentialId: "one" } }));
  expect(opts.replaceEditorDocument).toHaveBeenCalledWith(expect.objectContaining({ providers: [expect.objectContaining({
    supported_models: expect.arrayContaining(catalog.models), credentials: [expect.objectContaining({ model_catalog: catalog,
      supported_models: expect.arrayContaining(catalog.models) })] })] }), true);
  expect(JSON.stringify(opts.replaceEditorDocument.mock.calls)).not.toContain("verified_models");
});

it("replaces a legacy OpenRouter generative discovery job with directory metadata", async () => {
  const legacy = structuredClone(document);
  legacy.providers[0].preset = "openai";
  Object.assign(legacy.providers[0].credentials[0], { discovery_job: { id: "old-job", status: "pending" } });
  const opts = { client: { request: vi.fn().mockResolvedValue({ revision: "r1", catalog }) } as unknown as GatewayApiClient,
    managementToken: "fixture", secretGrant: "grant", editorText: JSON.stringify(legacy), revision: "r1", draftDirty: false,
    replaceEditorDocument: vi.fn(), requestSecretAccess: vi.fn(), setError: vi.fn() };
  const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("or", "one"));
  expect(opts.client.request).toHaveBeenCalledExactlyOnceWith("/v1/internal/gateway/console/openrouter-model-catalog", expect.anything(), expect.anything());
  expect(opts.replaceEditorDocument.mock.calls[0][0].providers[0].credentials[0]).not.toHaveProperty("discovery_job");
});
