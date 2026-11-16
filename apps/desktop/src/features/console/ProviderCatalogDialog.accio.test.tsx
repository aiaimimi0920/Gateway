import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
it("exposes the real Accio template with a unique ID, immutable target and no first credential", () => {
  const onSubmit = vi.fn(); const onRequestSecretAccess = vi.fn();
  render(<ProviderCatalogDialog open existingProviderIds={["accio-oauth", "accio-oauth-2"]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={onRequestSecretAccess} onSubmit={onSubmit} />, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^accio-oauth/ }));
  expect(screen.getByLabelText("Provider ID")).toHaveValue("accio-oauth-3");
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://phoenix-gw.alibaba.com");
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({
    templateId: "accio-oauth", providerId: "accio-oauth-3",
    supportedModels: [], secrets: {}, workflow: expect.objectContaining({ specialAdapter: "accio" })
  }));
  expect(onSubmit.mock.calls[0][0].credentialId).toBeUndefined(); expect(onRequestSecretAccess).not.toHaveBeenCalled();
});
