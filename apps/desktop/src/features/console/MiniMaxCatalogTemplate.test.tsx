import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { MINIMAX_TEMPLATES } from "./minimaxCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { catalogProviderKind } from "./officialModelCatalog";
import { templateAdaptationComplete } from "./providerTemplateAdaptation";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it.each(MINIMAX_TEMPLATES)("creates $id with a fixed region and no invented models or live receipt", async (template) => {
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: template.providerId }));
  expect(screen.getByLabelText("Base URL")).toHaveValue(template.baseUrl);
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "添加" }));
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, onSubmit.mock.calls[0][0]);
  expect(result.providers[0]).toMatchObject({ preset: template.id, base_url: template.baseUrl,
    explicit_credentials_only: true, credentials: [], supported_models: [],
    test_policy: { automaticEnabled: false, models: [] } });
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.model_routes).toEqual([]);
  expect(catalogProviderKind(result.providers[0] as Record<string, unknown>)).toBeUndefined();
  expect(templateAdaptationComplete(template.id)).toBe(false);
});

it("saves an explicit subscription Key and manual model without running discovery", async () => {
  const onSubmit = vi.fn(); const discover = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "minimax", label: "MiniMax M Plan" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => undefined} onDiscoverProviderModels={discover} onDiscover={generic} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "subscription" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "synthetic-minimax" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "MiniMax-M3.1-Flash" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "subscription",
    apiKeyValue: "synthetic-minimax", apiKeyOperation: "replace", manualModelsText: "MiniMax-M3.1-Flash" })));
  expect(discover).not.toHaveBeenCalled(); expect(generic).not.toHaveBeenCalled();
});
