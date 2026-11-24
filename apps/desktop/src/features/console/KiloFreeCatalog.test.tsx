import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { KILO_FREE_BASE, KILO_FREE_MODEL, KILO_FREE_TEMPLATE } from "./kiloFreeCatalog";

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
