import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { CURSOR_TEMPLATE } from "./cursorCatalog";
import { buildRouteAccountCatalog } from "./routeAccountCatalog";
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
const draft = (): ProviderCatalogDraft => ({ templateId: CURSOR_TEMPLATE.id, providerId: "cursor-remote", providerLabel: "Cursor",
  vendorKey: "cursor", vendorName: "Cursor", baseUrl: "", supportedModels: [] });
it("shows only the explicitly enabled Cursor credential in a draft, without changing other providers' defaults", () => {
  const { accounts } = buildRouteAccountCatalog({ providers: [
    { id: "personal", auth_mode: "cursor_remote", credentials: [
      { id: "selected", enabled: true }, { id: "paused", enabled: false }, { id: "unset" },
    ] },
    { id: "other", credentials: [{ id: "ordinary" }] },
  ], model_routes: [], aliases: {} });
  expect(Object.fromEntries(accounts.map((a) => [a.id, a.enabled])))
    .toEqual({ selected: true, paused: false, unset: false, ordinary: true });
});
it("creates only an import-only empty pool and never generic routes or credentials", () => {
  expect(PROVIDER_CATALOG_TEMPLATES).toContain(CURSOR_TEMPLATE);
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, CURSOR_TEMPLATE, draft());
  expect(result.providers).toEqual([{ id: "cursor-remote", label: "Cursor", vendor_key: "cursor", vendor_name: "Cursor",
    base_url: "", auth_mode: "cursor_remote", explicit_credentials_only: true, credentials: [] }]);
  expect(result.model_routes).toEqual([]);
  for (const patch of [{ baseUrl: "https://example.invalid" }, { apiKey: "secret" }, { supportedModels: ["default"] }]) {
    expect(() => addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, CURSOR_TEMPLATE, { ...draft(), ...patch })).toThrow();
  }
});
it("offers a real catalog entry without generic API Key, model, probe or workflow controls", () => {
  const submit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]} locked={false}
    hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^cursor-remote/ }));
  expect(screen.getByText(/此入口不提供官方独立模型 API/)).toHaveTextContent("也不会创建 Cloud Agent");
  expect(screen.queryByLabelText("Base URL")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  expect(screen.getAllByRole("tab").map(t => t.textContent)).toEqual(["基础与账号"]);
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).toHaveBeenCalledOnce();
  expect(() => addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, CURSOR_TEMPLATE, submit.mock.calls[0][0])).not.toThrow();
  expect(submit.mock.calls[0][0].workflow).toBeUndefined();
});
