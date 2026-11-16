import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { templateAdaptationComplete } from "./providerTemplateAdaptation";
import { sapBase, sapKey, sapProvider } from "./sapAiCoreTestFixtures";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("creates an explicit SAP deployment pool with required resource group and API version and no automatic calls", async () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "sap-aicore-chat");
  expect(template).toBeDefined(); expect(templateAdaptationComplete("sap-aicore-chat")).toBe(false);
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "sap-aicore-chat-api" }));
  expect(screen.getByLabelText("Base URL")).not.toHaveAttribute("readonly");
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: sapBase } });
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("AI-Resource-Group"), { target: { value: "group-a" } });
  fireEvent.change(screen.getByLabelText("API Version"), { target: { value: "2024-10-21" } });
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledTimes(1);
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, onSubmit.mock.calls[0][0]);
  expect(result.providers[0]).toMatchObject({ preset: "sap-aicore-chat", base_url: sapBase, auth_mode: "sap_aicore_service_key",
    headers: { "AI-Resource-Group": "group-a" }, extra_body: { sap_api_version: "2024-10-21" },
    explicit_credentials_only: true, credentials: [], supported_models: ["deployment1"],
    test_policy: { automaticEnabled: false, models: [] } });
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.model_routes).toEqual([]);
});

it("keeps invalid SAP material in the dialog, then submits a service key without discovery", async () => {
  const onSubmit = vi.fn(); const discover = vi.fn(); const onOpenChange = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "sap", label: "SAP" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={onOpenChange} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} sapProvider={() => sapProvider} onDiscover={discover} onDiscoverProviderModels={discover} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  expect(screen.getByLabelText("部署 ID")).toHaveValue("deployment1");
  expect(screen.getByLabelText("部署 ID")).toHaveAttribute("readonly");
  expect(screen.queryByRole("button", { name: "拉取模型" })).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("Service Key JSON"), { target: { value: "{}" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).not.toHaveBeenCalled(); expect(onOpenChange).not.toHaveBeenCalled();
  expect(await screen.findByText(/Service Key 必须包含/)).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Service Key JSON"), { target: { value: sapKey } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyValue: sapKey, supportedModelsText: "deployment1", baseUrl: sapBase }));
  expect(discover).not.toHaveBeenCalled();
});
