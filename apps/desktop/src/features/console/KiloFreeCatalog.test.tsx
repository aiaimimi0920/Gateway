import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { CredentialDialog } from "./CredentialDialog";
import { type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { KILO_FREE_BASE, KILO_FREE_MODEL, KILO_FREE_TEMPLATE } from "./kiloFreeCatalog";
import { VERCEL_PRESET, vercelModels } from "./vercelFreeCatalog";

it("adds a callable anonymous Kilo route without prompting for credentials", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} />
  </UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "kilo-free" }));
  expect(screen.getByLabelText("Base URL")).toHaveValue(KILO_FREE_BASE);
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  expect(screen.getByText(/无需账号或 API Key/)).toBeInTheDocument();
  expect(screen.queryByRole("checkbox", { name: "同时添加首个账号" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "添加" }));
  const draft = onSubmit.mock.calls[0][0] as ProviderCatalogDraft;
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, KILO_FREE_TEMPLATE, draft);
  expect(result.providers).toEqual([expect.objectContaining({
    preset: "kilo-free", base_url: KILO_FREE_BASE,
    explicit_credentials_only: false, credentials: [], supported_models: [KILO_FREE_MODEL],
  })]);
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.providers[0]).not.toHaveProperty("call_flow");
  expect(result.providers[0]).not.toHaveProperty("request_client_profile");
  expect(result.model_routes).toEqual([{ pattern: KILO_FREE_MODEL, provider_ids: ["kilo-free"] }]);
});

it("retains Vercel monthly credit mode and recommended models after entering an API key", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add"
    providerOptions={[{ id: "vercel", label: "Vercel AI Gateway", preset: VERCEL_PRESET }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess
    onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} />
  </UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.selectOptions(screen.getByLabelText("Vercel 额度模式"), "monthly");
  await user.click(screen.getByRole("button", { name: "添加推荐的 Vercel 模型" }));
  await user.type(screen.getByLabelText("账号 ID"), "vercel-team");
  await user.type(screen.getByLabelText("API Key"), "synthetic-vercel-key");
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({
    credentialId: "vercel-team", vercelMonthlyCredit: true,
    supportedModelsText: vercelModels(true).join("\n"),
  }));
});
