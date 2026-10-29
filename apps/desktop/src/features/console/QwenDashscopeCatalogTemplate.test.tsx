import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { templateAdaptationComplete } from "./providerTemplateAdaptation";

it("does not offer the retired Qwen DashScope template for provider creation", () => {
  for (const field of ["id", "providerId", "preset"] as const) {
    expect(PROVIDER_CATALOG_TEMPLATES.map((template) => template[field])).not.toContain("qwen-dashscope-openai");
  }
  expect(templateAdaptationComplete("qwen-dashscope-openai")).toBe(false);
});

it("hides Qwen DashScope in the template UI while retaining neighboring providers", () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={vi.fn()} /></UiLocaleProvider>);
  expect(screen.queryByRole("button", { name: /^qwen-dashscope-openai/i })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^mistral-api(?:\s*新调用流程已验收)?$/ })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^xAI-api(?:\s*新调用流程已验收)?$/ })).toBeInTheDocument();
});
