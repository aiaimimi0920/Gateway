import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { templateAdaptationComplete } from "./providerTemplateAdaptation";
import type { SapDeploymentCatalog } from "../../api/sapAiCoreDeployments";
import { sapEndpoint } from "./sapAiCore";
import { sapAzureBase, sapAzureKey, sapBase, sapKey, sapProvider } from "./sapAiCoreTestFixtures";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("limits the Azure address to the documented origin and deployment path", () => {
  expect(sapEndpoint(sapAzureBase)).toEqual({ origin: new URL(sapAzureBase).origin, deployment: "deployment1" });
  for (const refused of [
    sapAzureBase.replace("eu20", "eu21"), sapAzureBase.replace("westeurope", "northeurope"),
    sapAzureBase.replace("azure.ml", "gcp.ml"), sapAzureBase.replace(".com/", ".com.evil.invalid/"),
    sapAzureBase.replace(".com/", ".com:443/"), sapAzureBase.replace(".com/", ".com./"),
    sapAzureBase.replace("https://", "https://user@"), `${sapAzureBase}?x=1`, `${sapAzureBase}#x`, `${sapAzureBase}/`,
  ]) expect(sapEndpoint(refused), refused).toBeNull();
});

it.each([sapBase, sapAzureBase])("creates an explicit deployment pool for %s with no automatic calls", async (deploymentBase) => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "sap-aicore-chat");
  expect(template).toBeDefined(); expect(templateAdaptationComplete("sap-aicore-chat")).toBe(false);
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "sap-aicore-chat-api" }));
  expect(screen.getByLabelText("Base URL")).not.toHaveAttribute("readonly");
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: deploymentBase } });
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("AI-Resource-Group"), { target: { value: "group-a" } });
  fireEvent.change(screen.getByLabelText("API Version"), { target: { value: "2024-10-21" } });
  expect(screen.getByRole("checkbox", { name: "启用 GPT-4o 图片输入" })).not.toBeChecked();
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledTimes(1);
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, onSubmit.mock.calls[0][0]);
  expect(result.providers[0]).toMatchObject({ preset: "sap-aicore-chat", base_url: deploymentBase, auth_mode: "sap_aicore_service_key",
    headers: { "AI-Resource-Group": "group-a" }, extra_body: { sap_api_version: "2024-10-21" },
    explicit_credentials_only: true, credentials: [], supported_models: ["deployment1"],
    test_policy: { automaticEnabled: false, models: [] } });
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.providers[0]).not.toHaveProperty("extra_body.sap_image_input");
  expect(result.model_routes).toEqual([]);
});

it.each([
  { base: sapBase, key: sapKey, foreignKey: sapAzureKey },
  { base: sapAzureBase, key: sapAzureKey, foreignKey: sapKey },
])("rejects a foreign Service Key and accepts the bound key for $base", async ({ base, key, foreignKey }) => {
  const onSubmit = vi.fn(); const discover = vi.fn(); const onOpenChange = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "sap", label: "SAP" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={onOpenChange} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} sapProvider={() => ({ ...sapProvider, base_url: base })} onDiscover={discover} onDiscoverProviderModels={discover} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  expect(screen.getByLabelText("部署 ID")).toHaveValue("deployment1");
  expect(screen.getByLabelText("部署 ID")).toHaveAttribute("readonly");
  expect(screen.queryByRole("button", { name: "拉取模型" })).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("Service Key JSON"), { target: { value: foreignKey } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).not.toHaveBeenCalled(); expect(onOpenChange).not.toHaveBeenCalled();
  expect(await screen.findByText(/Service Key 必须包含/)).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Service Key JSON"), { target: { value: key } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyValue: key, supportedModelsText: "deployment1", baseUrl: base }));
  expect(discover).not.toHaveBeenCalled();
});

it("looks up SAP deployments only by explicit request and does not select or persist the discovery secret", async () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "sap-aicore-chat")!;
  const second = sapBase.replace("deployment1", "deployment2");
  const discover = vi.fn().mockResolvedValue({ deployments: [
    { id: "deployment1", deploymentUrl: sapBase, modelName: "gpt-4o", modelVersion: "2024-11-20" },
    { id: "deployment2", deploymentUrl: second, modelName: "gpt-4.1" },
  ], truncated: false });
  const onSubmit = vi.fn(); const user = userEvent.setup();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit}
    onDiscoverSapDeployments={discover} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "sap-aicore-chat-api" }));
  fireEvent.change(screen.getByLabelText("AI-Resource-Group"), { target: { value: "group-a" } });
  fireEvent.change(screen.getByLabelText("API Version"), { target: { value: "2024-10-21" } });
  fireEvent.change(screen.getByLabelText("查询用 Service Key JSON"), { target: { value: sapKey } });
  expect(discover).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "查询部署" }));
  expect(discover).toHaveBeenCalledWith({ serviceKey: sapKey, resourceGroup: "group-a" }, expect.any(AbortSignal));
  expect(screen.getByLabelText("查询用 Service Key JSON")).toHaveValue("");
  expect(screen.getByLabelText("Base URL")).toHaveValue("");
  const choice = await screen.findByLabelText("选择部署");
  expect(choice).toHaveValue("");
  await user.selectOptions(choice, "deployment2");
  expect(screen.getByLabelText("Base URL")).toHaveValue(second);
  expect(screen.getByRole("checkbox", { name: "启用 GPT-4o 图片输入" })).not.toBeChecked();
  expect(onSubmit).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("AI-Resource-Group"), { target: { value: "group-b" } });
  expect(screen.queryByLabelText("选择部署")).not.toBeInTheDocument();
  expect(screen.getByLabelText("Base URL")).toHaveValue("");
  // The manual path is still available; no stale discovery result is saved by accident.
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: second } });
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledTimes(1);
  const draft = onSubmit.mock.calls[0][0];
  expect(JSON.stringify(draft)).not.toContain(sapKey);
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, draft);
  expect(result.providers[0]).toMatchObject({ base_url: second, supported_models: ["deployment2"], credentials: [],
    headers: { "AI-Resource-Group": "group-b" } });
});

it("persists image input only after explicitly enabling it on a selected URL and stable API version", async () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "sap-aicore-chat")!;
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "sap-aicore-chat-api" }));
  const checkbox = screen.getByRole("checkbox", { name: "启用 GPT-4o 图片输入" });
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: sapBase } });
  fireEvent.change(screen.getByLabelText("AI-Resource-Group"), { target: { value: "group-a" } });
  fireEvent.change(screen.getByLabelText("API Version"), { target: { value: "2023-05-15" } });
  expect(checkbox).toBeDisabled();
  fireEvent.change(screen.getByLabelText("API Version"), { target: { value: "2024-10-21" } });
  await user.click(checkbox);
  expect(checkbox).toBeChecked();
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: sapBase.replace("deployment1", "deployment2") } });
  expect(checkbox).not.toBeChecked();
  await user.click(checkbox);
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledTimes(1);
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, onSubmit.mock.calls[0][0]);
  expect(result.providers[0]).toHaveProperty("extra_body", { sap_api_version: "2024-10-21", sap_image_input: true });
});

it.each(["resource group", "protocol"])("cancels an in-flight lookup on %s change and drops its late result", async (scope) => {
  let finish!: (value: SapDeploymentCatalog) => void;
  const lookup = vi.fn((_body: unknown, _signal: AbortSignal) => new Promise<SapDeploymentCatalog>((resolve) => { finish = resolve; }));
  const submit = vi.fn(); const user = userEvent.setup();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={submit}
    onDiscoverSapDeployments={lookup} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "sap-aicore-chat-api" }));
  fireEvent.change(screen.getByLabelText("AI-Resource-Group"), { target: { value: "group-a" } });
  fireEvent.change(screen.getByLabelText("查询用 Service Key JSON"), { target: { value: sapKey } });
  await user.click(screen.getByRole("button", { name: "查询部署" }));
  const signal = lookup.mock.calls[0][1];
  expect(signal.aborted).toBe(false);
  if (scope === "resource group") {
    fireEvent.change(screen.getByLabelText("AI-Resource-Group"), { target: { value: "group-b" } });
  } else {
    await user.click(screen.getByRole("checkbox", { name: "使用 Bedrock Converse" }));
    expect(screen.getByLabelText("查询用 Service Key JSON")).toHaveValue("");
  }
  expect(signal.aborted).toBe(true);
  await act(async () => finish({ deployments: [{ id: "deployment1", deploymentUrl: sapBase, modelName: "gpt-4o" }], truncated: false }));
  expect(screen.queryByLabelText("选择部署")).not.toBeInTheDocument();
  expect(screen.getByLabelText("Base URL")).toHaveValue("");
  expect(submit).not.toHaveBeenCalled();
});

it("binds o3-mini reasoning to an explicit deployment and its preview API version", async () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "sap-aicore-chat")!;
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "sap-aicore-chat-api" }));
  const checkbox = screen.getByRole("checkbox", { name: "启用 o3-mini 推理参数" });
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: sapBase } });
  fireEvent.change(screen.getByLabelText("AI-Resource-Group"), { target: { value: "group-a" } });
  fireEvent.change(screen.getByLabelText("API Version"), { target: { value: "2024-10-21" } });
  expect(checkbox).toBeDisabled();
  fireEvent.change(screen.getByLabelText("API Version"), { target: { value: "2024-12-01-preview" } });
  await user.click(checkbox);
  expect(checkbox).toBeChecked();
  fireEvent.change(screen.getByLabelText("AI-Resource-Group"), { target: { value: "group-b" } });
  expect(checkbox).not.toBeChecked();
  await user.click(checkbox);
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: sapBase.replace("deployment1", "deployment2") } });
  expect(checkbox).not.toBeChecked();
  await user.click(checkbox);
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledTimes(1);
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, onSubmit.mock.calls[0][0]);
  expect(result.providers[0]).toMatchObject({ credentials: [], supported_models: ["deployment2"],
    extra_body: { sap_api_version: "2024-12-01-preview", sap_o3_mini: true } });
});

it("saves native stateless Responses only after opt-in for the current SAP deployment binding", async () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "sap-aicore-chat")!;
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "sap-aicore-chat-api" }));
  const checkbox = screen.getByRole("checkbox", { name: "启用原生无状态 Responses" });
  const images = screen.getByRole("checkbox", { name: "启用原生 Responses 图片输入" });
  const stored = screen.getByRole("checkbox", { name: "启用持久化 Responses" });
  expect(checkbox).toBeDisabled();
  expect(images).toBeDisabled();
  expect(stored).toBeDisabled();
  const base = screen.getByLabelText("Base URL");
  const group = screen.getByLabelText("AI-Resource-Group");
  fireEvent.change(base, { target: { value: sapBase } });
  fireEvent.change(group, { target: { value: "group-a" } });
  const version = screen.getByLabelText("API Version");
  fireEvent.change(version, { target: { value: "2024-10-21" } });
  expect(checkbox).not.toBeChecked();
  await user.click(checkbox);
  expect(checkbox).toBeChecked();
  expect(images).not.toBeChecked();
  expect(stored).not.toBeChecked();
  await user.click(images);
  await user.click(stored);
  expect(images).toBeChecked();
  fireEvent.change(group, { target: { value: "group-b" } });
  expect(checkbox).not.toBeChecked();
  expect(images).toBeDisabled();
  expect(images).not.toBeChecked();
  expect(stored).not.toBeChecked();
  expect(stored).toBeDisabled();
  await user.click(checkbox);
  await user.click(images);
  await user.click(stored);
  fireEvent.change(version, { target: { value: "2024-12-01-preview" } });
  expect(checkbox).not.toBeChecked();
  expect(images).not.toBeChecked();
  expect(stored).not.toBeChecked();
  await user.click(checkbox);
  await user.click(images);
  await user.click(stored);
  fireEvent.change(base, { target: { value: sapBase.replace("deployment1", "deployment2") } });
  expect(checkbox).not.toBeChecked();
  expect(images).not.toBeChecked();
  expect(stored).not.toBeChecked();
  fireEvent.change(base, { target: { value: sapBase } });
  expect(checkbox).not.toBeChecked();
  await user.click(checkbox);
  await user.click(images);
  await user.click(stored);
  await user.click(checkbox);
  expect(images).not.toBeChecked();
  expect(images).toBeDisabled();
  expect(stored).not.toBeChecked();
  expect(stored).toBeDisabled();
  await user.click(checkbox);
  await user.click(images);
  await user.click(stored);
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledTimes(1);
  const saved = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, onSubmit.mock.calls[0][0]);
  expect(saved.providers[0]).toMatchObject({ credentials: [], headers: { "AI-Resource-Group": "group-b" },
    extra_body: { sap_api_version: "2024-12-01-preview", sap_native_responses: true, sap_native_responses_image_input: true, sap_stored_responses: true } });
});

it("saves a fixed Orchestration model with no Chat API version", async () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "sap-aicore-chat")!;
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "sap-aicore-chat-api" }));
  const base = screen.getByLabelText("Base URL");
  const group = screen.getByLabelText("AI-Resource-Group");
  fireEvent.change(base, { target: { value: sapBase } });
  fireEvent.change(group, { target: { value: "group-a" } });
  fireEvent.change(screen.getByLabelText("API Version"), { target: { value: "2024-10-21" } });
  await user.click(screen.getByRole("checkbox", { name: "启用原生无状态 Responses" }));
  await user.click(screen.getByRole("checkbox", { name: "使用 Orchestration v2" }));
  expect(screen.queryByLabelText("API Version")).not.toBeInTheDocument();
  expect(screen.queryByRole("checkbox", { name: "启用原生无状态 Responses" })).not.toBeInTheDocument();
  const model = screen.getByLabelText("Orchestration 模型");
  fireEvent.change(model, { target: { value: "gpt-4.1-mini" } });
  fireEvent.change(group, { target: { value: "group-b" } });
  expect(model).toHaveValue("");
  fireEvent.change(model, { target: { value: "gpt-4.1-mini" } });
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledTimes(1);
  const saved = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, onSubmit.mock.calls[0][0]);
  expect(saved.providers[0]).toMatchObject({ credentials: [], supported_models: ["deployment1"], headers: { "AI-Resource-Group": "group-b" } });
  expect(saved.providers[0]).toHaveProperty("extra_body", { sap_orchestration_model: "gpt-4.1-mini" });
});

it.each(["manual", "catalog"])("saves a %s Bedrock Converse deployment without Azure settings", async (source) => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "sap-aicore-chat")!;
  const user = userEvent.setup(); const onSubmit = vi.fn();
  const second = sapBase.replace("deployment1", "bedrock-2");
  const discover = vi.fn().mockResolvedValue({ deployments: [
    { id: "bedrock-2", deploymentUrl: second, configurationName: "my-bedrock" },
    { id: "mismatch", deploymentUrl: sapBase },
  ], truncated: false });
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={true} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit}
    onDiscoverSapDeployments={discover} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "sap-aicore-chat-api" }));
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: sapBase } });
  fireEvent.change(screen.getByLabelText("AI-Resource-Group"), { target: { value: "group-a" } });
  fireEvent.change(screen.getByLabelText("API Version"), { target: { value: "2024-10-21" } });
  await user.click(screen.getByRole("checkbox", { name: "启用原生无状态 Responses" }));
  expect(screen.getByLabelText("查询用 Service Key JSON")).toBeInTheDocument();
  await user.click(screen.getByRole("checkbox", { name: "使用 Bedrock Converse" }));
  expect(screen.queryByLabelText("API Version")).not.toBeInTheDocument();
  expect(screen.queryByRole("checkbox", { name: "启用原生无状态 Responses" })).not.toBeInTheDocument();
  expect(screen.getByRole("checkbox", { name: "使用 Orchestration v2" })).not.toBeChecked();
  expect(discover).not.toHaveBeenCalled();
  const base = screen.getByLabelText("Base URL");
  if (source === "catalog") {
    fireEvent.change(screen.getByLabelText("查询用 Service Key JSON"), { target: { value: sapKey } });
    await user.click(screen.getByRole("button", { name: "查询部署" }));
    expect(discover).toHaveBeenCalledWith({ serviceKey: sapKey, resourceGroup: "group-a", protocol: "bedrock" }, expect.any(AbortSignal));
    expect(screen.getByLabelText("查询用 Service Key JSON")).toHaveValue("");
    const choice = await screen.findByLabelText("选择已确认支持 Converse 的部署");
    expect(choice).toHaveValue("");
    expect(screen.getByRole("option", { name: "my-bedrock · bedrock-2" })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /mismatch/ })).not.toBeInTheDocument();
    expect(base).toHaveValue(sapBase);
    expect(onSubmit).not.toHaveBeenCalled();
    await user.selectOptions(choice, "bedrock-2");
    expect(base).toHaveValue(second);
  }
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledTimes(1);
  const submitted = onSubmit.mock.calls[0][0];
  expect(JSON.stringify(submitted)).not.toContain(sapKey);
  const saved = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, submitted);
  expect(saved.providers[0]).toMatchObject({ credentials: [], base_url: source === "catalog" ? second : sapBase,
    supported_models: [source === "catalog" ? "bedrock-2" : "deployment1"], headers: { "AI-Resource-Group": "group-a" },
    extra_body: { sap_bedrock_converse: true } });
});
