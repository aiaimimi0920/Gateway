import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { templateAdaptationComplete } from "./providerTemplateAdaptation";

it("does not offer the retired Together template for provider creation or mark it accepted", () => {
  expect(PROVIDER_CATALOG_TEMPLATES.map(({ id }) => id)).not.toContain("together-openai");
  expect(PROVIDER_CATALOG_TEMPLATES.map(({ providerId }) => providerId)).not.toContain("together");
  expect(PROVIDER_CATALOG_TEMPLATES.map(({ preset }) => preset)).not.toContain("together-openai");
  expect(templateAdaptationComplete("together-openai")).toBe(false);
});

it("hides Together in the standard-template UI while retaining neighboring providers", () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={vi.fn()} /></UiLocaleProvider>);
  expect(screen.queryByRole("button", { name: /^together/i })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^nvidia-api(?:\s*新调用流程已验收)?$/i })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^deepseek-api(?:\s*新调用流程已验收)?$/ })).toBeInTheDocument();
});
