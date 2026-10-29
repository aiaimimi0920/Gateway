import { act, render, renderHook, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { registerChatgptCredentialModels } from "./chatgptCredentialModels";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";

const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === "chatgpt")!;

it("creates the fixed native Codex template without importing an account or enabling Basispoints", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} />
  </UiLocaleProvider>);
  await user.click(screen.getByRole("button", { name: /^codex-oauth(?:新调用流程已验收)?$/ }));
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://chatgpt.com/backend-api/codex");
  for (const label of ["Base URL", "调用适配器", "指纹适配器", "专用适配器"]) {
    expect(screen.getByLabelText(label)).toHaveAttribute("readonly");
  }
  expect(screen.getByLabelText("调用适配器")).toHaveValue("OpenAI Responses");
  expect(screen.getByLabelText("指纹适配器")).toHaveValue("Codex");
  await user.click(screen.getByRole("button", { name: "添加" }));
  const draft = onSubmit.mock.calls[0][0] as ProviderCatalogDraft;
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, draft);
  expect(result.providers[0]).toMatchObject({ id: "codex-oauth", preset: "chatgpt-codex-oauth-official-api",
    base_url: "https://chatgpt.com/backend-api/codex", protocol_profile: "chatgpt_codex_backend",
    explicit_credentials_only: true, credentials: [], call_flow: {
      call_adapter: "auto", fingerprint_adapter: "codex_cli", special_adapter: "auto",
    }, test_policy: { automaticEnabled: false },
  });
  expect(result.providers[0]).not.toHaveProperty("codex_basispoints_enabled");
  expect(result.providers[0]).not.toHaveProperty("api_key");
});

it("stages the imported token separately from the exact account header without a refresh token", () => {
  const document = { providers: [{ id: "chatgpt", preset: template.preset, credentials: [] }], model_routes: [], aliases: {} };
  const replaceEditorDocument = vi.fn(); const setCredentialSecretEdits = vi.fn();
  const hook = renderHook(() => useCredentialDialogEditor({ editorText: JSON.stringify(document),
    credentialDialogState: { mode: "add", initialValue: null }, setCredentialDialogState: vi.fn(),
    setCredentialSecretEdits, setError: vi.fn(), replaceEditorDocument, t: (zh) => zh }));
  act(() => hook.result.current.applyCredentialDialogValue({ providerId: "chatgpt", credentialId: "imported",
    accountName: "Imported", enabled: true, baseUrl: "", supportedModelsText: "fixture-model",
    apiKeyOperation: "replace", apiKeyValue: "fixture-access-token", chatgptAccountId: "fixture-account", chatgptGroup: "pro20x" }));
  const saved = replaceEditorDocument.mock.calls[0][0];
  expect(saved.providers[0].credentials[0]).toMatchObject({ id: "imported", headers: { "Chatgpt-Account-Id": "fixture-account" } });
  expect(saved.providers[0].credentials[0].credential_identity_category_id).toBe("pro20x");
  expect(saved.providers[0]).toMatchObject({ supported_models: ["fixture-model"], default_model: "fixture-model" });
  expect(saved.model_routes).toEqual([{ pattern: "fixture-model", provider_ids: ["chatgpt"] }]);
  expect(JSON.stringify(saved)).not.toContain("fixture-access-token");
  expect(JSON.stringify(saved)).not.toContain("refresh_token");
  expect(setCredentialSecretEdits.mock.calls[0][0]([])).toEqual([{ providerId: "chatgpt", credentialId: "imported",
    field: "api_key", operation: "replace", value: "fixture-access-token" }]);
});

it("preserves an existing OAuth account's headers and refresh material during an ordinary edit", () => {
  const document = { providers: [{ id: "chatgpt", preset: template.preset, credentials: [{ id: "existing",
    headers: { "Chatgpt-Account-Id": "original-account" }, refresh_token: "fixture-refresh", refresh_endpoint: "https://fixture.test/token" }] }],
    model_routes: [], aliases: {} };
  const replaceEditorDocument = vi.fn();
  const hook = renderHook(() => useCredentialDialogEditor({ editorText: JSON.stringify(document),
    credentialDialogState: { mode: "edit", initialValue: null }, setCredentialDialogState: vi.fn(),
    setCredentialSecretEdits: vi.fn(), setError: vi.fn(), replaceEditorDocument, t: (zh) => zh }));
  act(() => hook.result.current.applyCredentialDialogValue({ providerId: "chatgpt", credentialId: "existing",
    accountName: "Renamed", enabled: true, baseUrl: "", supportedModelsText: "", apiKeyOperation: "keep", apiKeyValue: "",
    chatgptAccountId: "must-not-overwrite" }));
  expect(replaceEditorDocument.mock.calls[0][0].providers[0].credentials[0]).toMatchObject({
    account_name: "Renamed", headers: { "Chatgpt-Account-Id": "original-account" },
    refresh_token: "fixture-refresh", refresh_endpoint: "https://fixture.test/token",
  });
});

it("registers declared models idempotently without replacing route policies or other providers", () => {
  const document = { providers: [{ id: "chatgpt", preset: template.preset, supported_models: ["older"],
    credentials: [{ id: "imported", credential_identity_category_id: "pro20x" }] }], model_routes: [
    { pattern: "shared", provider_ids: ["other"], priority: 7, enabled: false },
  ], aliases: {} };
  const once = registerChatgptCredentialModels(document, "chatgpt", ["shared", "shared"]);
  expect(registerChatgptCredentialModels(once, "chatgpt", ["shared"])).toEqual(once);
  expect(once.model_routes).toEqual([{ pattern: "shared", provider_ids: ["other", "chatgpt"], priority: 7, enabled: false }]);
  expect(once.providers[0]).toMatchObject({ supported_models: ["older", "shared"] });
  expect(document.model_routes[0].provider_ids).toEqual(["other"]);
  expect(credentialDialogValueFromDocument(once, "chatgpt", "imported")?.chatgptGroup).toBe("pro20x");
});
