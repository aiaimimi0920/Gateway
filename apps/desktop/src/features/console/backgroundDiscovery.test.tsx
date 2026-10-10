import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { ConsoleApi } from "../../api/console";
import type { ConsoleRouteConfigResponse } from "../../api/contracts";
import { useBackgroundDiscoverySync } from "./useBackgroundDiscoverySync";
import { useProviderCatalogEditor } from "./useProviderCatalogEditor";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";

afterEach(() => vi.useRealTimers());
const t = (zh: string) => zh;
const document = { providers: [{ id: "pool", adapter: "openai_compatible", base_url: "https://fixture.test",
  credentials: [{ id: "account" }] }], model_routes: [], aliases: {} };

it("creates a persisted discovery intent and leaves the key in the secret patch lane", () => {
  const replaceEditorDocument = vi.fn(); const updateCredentialSecretEdit = vi.fn();
  const hook = renderHook(() => useProviderCatalogEditor({ editorText: JSON.stringify({ providers: [], model_routes: [], aliases: {} }),
    replaceEditorDocument, updateCredentialSecretEdit, setActiveWorkspace: vi.fn(), setError: vi.fn(), t }));
  act(() => hook.result.current.applyProviderCatalogDraft({ templateId: "custom-api-provider", providerId: "pool",
    providerLabel: "Fixture", vendorKey: "fixture", vendorName: "Fixture", baseUrl: "https://fixture.test",
    supportedModels: [], credentialId: "account", accountName: "Fixture account", apiKey: "fixture-secret" }));
  const saved = replaceEditorDocument.mock.calls[0][0];
  expect(saved.providers[0].credentials[0]).toMatchObject({ id: "account",
    discovery_job: { id: expect.any(String), status: "pending" } });
  expect(saved.providers[0].credentials[0].discovery).toBeUndefined();
  expect(JSON.stringify(saved)).not.toContain("fixture-secret");
  expect(updateCredentialSecretEdit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyValue: "fixture-secret" }));
});

it("queues new key detection while normal edits preserve the current job", () => {
  const replaceEditorDocument = vi.fn();
  const hook = renderHook(() => useCredentialDialogEditor({ editorText: JSON.stringify(document),
    credentialDialogState: { mode: "edit", initialValue: null }, setCredentialDialogState: vi.fn(),
    setCredentialSecretEdits: vi.fn(), setError: vi.fn(), replaceEditorDocument, t }));
  act(() => hook.result.current.applyCredentialDialogValue({ providerId: "pool", credentialId: "account",
    accountName: "renamed", enabled: true, baseUrl: "", supportedModelsText: "",
    apiKeyOperation: "replace", apiKeyValue: "replacement-secret" }));
  expect(replaceEditorDocument.mock.calls[0][0].providers[0].credentials[0].discovery_job.status).toBe("pending");
  expect(JSON.stringify(replaceEditorDocument.mock.calls[0][0])).not.toContain("replacement-secret");
});

function config(revision: string): ConsoleRouteConfigResponse {
  return { routeConfig: { revision: { id: revision }, document: {
    ...document, providers: [{ ...document.providers[0], credentials: [{ id: "account",
      discovery_job: { id: "job", status: revision === "initial" ? "pending" : "complete" } }] }],
  } } } as unknown as ConsoleRouteConfigResponse;
}

it("polls background results without global loading and skips dirty drafts", async () => {
  vi.useFakeTimers();
  const result = config("complete"); const getRouteConfig = vi.fn().mockResolvedValue(result);
  const setRouteConfig = vi.fn();
  const initial = { api: { getRouteConfig } as unknown as ConsoleApi, managementToken: "fixture-token",
    routeConfig: config("initial"), editorText: "initial text", blocked: true, setRouteConfig };
  const hook = renderHook((props) => useBackgroundDiscoverySync(props), { initialProps: initial });
  await act(() => vi.advanceTimersByTimeAsync(3000));
  expect(getRouteConfig).not.toHaveBeenCalled();
  hook.rerender({ ...initial, blocked: false });
  await act(() => vi.advanceTimersByTimeAsync(3000));
  expect(setRouteConfig).toHaveBeenCalledWith(result);
  hook.unmount();
  await act(() => vi.advanceTimersByTimeAsync(6000));
  expect(getRouteConfig).toHaveBeenCalledTimes(1);
});

it("does not overwrite edits or a changed session when a poll returns late", async () => {
  vi.useFakeTimers();
  let resolve!: (value: ConsoleRouteConfigResponse) => void;
  const getRouteConfig = vi.fn(() => new Promise<ConsoleRouteConfigResponse>((done) => { resolve = done; }));
  const setRouteConfig = vi.fn();
  const initial = { api: { getRouteConfig } as unknown as ConsoleApi, managementToken: "fixture-token",
    routeConfig: config("initial"), editorText: "initial text", blocked: false, setRouteConfig };
  const hook = renderHook((props) => useBackgroundDiscoverySync(props), { initialProps: initial });
  await act(() => vi.advanceTimersByTimeAsync(3000));
  hook.rerender({ ...initial, editorText: "user editing", blocked: true });
  await act(async () => resolve(config("complete")));
  expect(setRouteConfig).not.toHaveBeenCalled();
  hook.rerender({ ...initial, managementToken: "new-session" });
  await act(() => vi.advanceTimersByTimeAsync(3000));
  hook.unmount();
  await act(async () => resolve(config("complete")));
  expect(setRouteConfig).not.toHaveBeenCalled();
});
