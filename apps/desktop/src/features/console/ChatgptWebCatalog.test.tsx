import { act, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { useAccountDiscovery } from "./useAccountDiscovery";

const catalog = { checkedAt: "now", models: [{ slug: "actual-web-slug", visibility: null, supported_in_api: null }] };
const initialValue = { providerId: "web", credentialId: "account", accountName: "", baseUrl: "", enabled: true,
  supportedModelsText: "manual", apiKeyOperation: "keep" as const, apiKeyValue: "" };

it("explicitly fetches an existing Web account by ID and stages fetched models without replacing manual models", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const discover = vi.fn().mockResolvedValue(catalog); const submit = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="edit" providerOptions={[{ id: "web", label: "chatgpt-web" }]}
    initialValue={initialValue} existingCredentialIds={["account"]} locked={false} hasSecretAccess
    supportsWebSession={() => true} supportsBasispoints={() => false} onSubmit={submit}
    onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onDiscoverChatgptModels={discover} /></UiLocaleProvider>);
  expect(screen.getByRole("button", { name: "拉取模型" })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "拉取模型" }));
  await waitFor(() => expect(screen.getByLabelText("拉取模型列表")).toHaveValue("actual-web-slug"));
  expect(discover).toHaveBeenCalledWith({ credentialId: "account" }, expect.any(AbortSignal));
  expect(screen.getByLabelText("手动模型列表")).toHaveValue("manual");
  expect(submit).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyOperation: "keep", supportedModelsText: "manual\nactual-web-slug" }));
});

it("allows manually specified Web models to be saved after a catalog rejection without claiming verification", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const submit = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="edit" providerOptions={[{ id: "web", label: "chatgpt-web" }]}
    initialValue={initialValue} existingCredentialIds={["account"]} locked={false} hasSecretAccess
    supportsWebSession={() => true} onSubmit={submit} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onDiscoverChatgptModels={vi.fn().mockRejectedValue(new Error("catalog HTTP 403"))} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "operator-selected-slug" } });
  fireEvent.click(screen.getByRole("button", { name: "拉取模型" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("catalog HTTP 403");
  expect(screen.getByLabelText("手动模型列表")).toHaveValue("operator-selected-slug");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ supportedModelsText: "operator-selected-slug", apiKeyOperation: "keep" }));
  expect(JSON.stringify(submit.mock.calls)).not.toContain("verified_models");
});

it("imports browser-read model slugs while preserving manual models without another catalog request", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const discover = vi.fn();
  const capture = vi.fn().mockResolvedValue({ accountName: "Web account", material: {
    authToken: "fixture-token", expiresAt: "2099-01-01T00:00:00Z", cookieHeader: "session=fixture",
    userAgent: "FixtureBrowser", deviceId: "fixture-device", modelSlugs: ["browser-model"],
  } });
  render(<UiLocaleProvider><CredentialDialog open mode="edit" providerOptions={[{ id: "web", label: "chatgpt-web" }]}
    initialValue={initialValue} existingCredentialIds={["account"]} locked={false} hasSecretAccess
    supportsWebSession={() => true} onSubmit={vi.fn()} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onDiscoverChatgptModels={discover} onCaptureWebBrowser={capture} /></UiLocaleProvider>);
  fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
  await screen.findByText("已读取账号：Web account");
  expect(screen.getByLabelText("拉取模型列表")).toHaveValue("browser-model");
  expect(screen.getByLabelText("手动模型列表")).toHaveValue("manual");
  expect(discover).not.toHaveBeenCalled();
});

it("refreshes Web catalog with the normal secret grant and revision fence, preserving disabled routes", async () => {
  const document = { providers: [{ id: "web", preset: "chatgpt-web-reverse", supported_models: ["manual"],
    credentials: [{ id: "account", supported_models: ["manual"], model_selection: { fetched: [], manual: ["manual"] } }] }],
    model_routes: [{ pattern: "actual-web-slug", provider_ids: ["other"], priority: 7, enabled: false }], aliases: {} };
  for (const revision of ["r1", "stale"]) {
    const request = vi.fn().mockResolvedValue({ revision, catalog }); const replaceEditorDocument = vi.fn();
    const hook = renderHook(() => useAccountDiscovery({ client: { request }, managementToken: "manager", secretGrant: "grant",
      editorText: JSON.stringify(document), revision: "r1", draftDirty: false, replaceEditorDocument,
      requestSecretAccess: vi.fn(), setError: vi.fn() }));
    await act(() => hook.result.current.refreshAccountDiscovery("web", "account"));
    expect(request).toHaveBeenCalledWith("/v1/internal/gateway/console/chatgpt-model-catalog", expect.anything(),
      expect.objectContaining({ managementToken: "manager", secretGrant: "grant", body: { credentialId: "account" }, signal: expect.any(AbortSignal) }));
    if (revision === "r1") {
      const saved = replaceEditorDocument.mock.calls[0][0];
      expect(saved.providers[0].credentials[0].model_selection).toEqual({ fetched: ["actual-web-slug"], manual: ["manual"] });
      expect(saved.model_routes[0]).toEqual({ pattern: "actual-web-slug", provider_ids: ["other", "web"], priority: 7, enabled: false });
      expect(JSON.stringify(saved)).not.toContain("verified_models");
    } else expect(replaceEditorDocument).not.toHaveBeenCalled();
    hook.unmount();
  }
});
