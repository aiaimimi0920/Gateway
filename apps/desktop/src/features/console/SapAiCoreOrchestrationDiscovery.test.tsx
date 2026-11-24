import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { sapBase, sapKey } from "./sapAiCoreTestFixtures";

it("selects an Orchestration deployment explicitly, without selecting a model or saving the lookup key", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const user = userEvent.setup();
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "sap-aicore-chat")!;
  const orchestrationUrl = sapBase.replace("deployment1", "orchestration-1");
  const discover = vi.fn().mockResolvedValue({ deployments: [
    { id: "orchestration-1", deploymentUrl: orchestrationUrl, configurationName: "workflow-a" },
  ], truncated: false });
  const submit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={submit} onDiscoverSapDeployments={discover} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "sap-aicore-chat-api" }));
  await user.click(screen.getByRole("checkbox", { name: "使用 Orchestration v2" }));
  fireEvent.change(screen.getByLabelText("AI-Resource-Group"), { target: { value: "group-a" } });
  const lookupKey = screen.getByLabelText("查询用 Service Key JSON");
  fireEvent.change(lookupKey, { target: { value: sapKey } });
  expect(discover).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "查询部署" }));
  expect(discover).toHaveBeenCalledWith({ serviceKey: sapKey, resourceGroup: "group-a", protocol: "orchestration" }, expect.any(AbortSignal));
  expect(lookupKey).toHaveValue("");
  const picker = await screen.findByLabelText("选择 Orchestration 部署");
  const base = screen.getByLabelText("Base URL");
  const model = screen.getByLabelText("Orchestration 模型");
  expect(picker).toHaveValue("");
  expect(base).toHaveValue("");
  fireEvent.change(model, { target: { value: "previous-model" } });
  await user.selectOptions(picker, "orchestration-1");
  expect(base).toHaveValue(orchestrationUrl);
  expect(model).toHaveValue("");
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).not.toHaveBeenCalled();
  fireEvent.change(model, { target: { value: "gpt-4.1-mini" } });
  const image = screen.getByRole("checkbox", { name: "启用 Orchestration 图片输入" });
  const reasoning = screen.getByRole("checkbox", { name: "启用 Orchestration 推理" });
  expect(reasoning).not.toBeChecked();
  await user.click(reasoning);
  expect(image).not.toBeChecked();
  await user.click(image);
  fireEvent.change(model, { target: { value: "gpt-4o" } });
  expect(image).not.toBeChecked();
  expect(reasoning).not.toBeChecked();
  await user.click(reasoning);
  await user.click(image);
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).toHaveBeenCalledTimes(1);
  const [draft] = submit.mock.calls[0];
  expect(JSON.stringify(draft)).not.toContain(sapKey);
  const saved = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, draft);
  expect(saved.providers[0]).toMatchObject({ base_url: orchestrationUrl, supported_models: ["orchestration-1"], credentials: [] });
  expect(saved.providers[0]).toHaveProperty("extra_body", { sap_orchestration_model: "gpt-4o", sap_orchestration_image_input: true, sap_orchestration_reasoning: true });
});
