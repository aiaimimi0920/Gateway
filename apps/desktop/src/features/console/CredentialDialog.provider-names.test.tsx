import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { parseRouteDocument } from "./routeDocument";
import { useConsoleCredentialSelectors } from "./useConsoleCredentialSelectors";

it("renders distinct OAuth/Web names while selecting the original saved provider ID", () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const document = parseRouteDocument(JSON.stringify({ providers: [
    { id: "original-codex-id", label: "chatgpt", preset: "chatgpt-codex-oauth-official-api" },
    { id: "original-web-id", label: "chatgpt", preset: "chatgpt-web-reverse" },
    { id: "nvidia-api", label: "nvidia-api", preset: "nvidia-openai" },
  ], model_routes: [], aliases: {} }));
  const before = JSON.stringify(document);
  function Fixture() {
    const { credentialProviderOptions } = useConsoleCredentialSelectors({
      credentialPoolAutomation: null, credentialRefill: null,
      draftDocumentState: { document, invalid: false },
      draftAccountCatalog: { groups: [], accounts: [], providerBuckets: [], ungroupedCount: 0 },
    });
    return <CredentialDialog open mode="add" providerOptions={credentialProviderOptions}
      existingCredentialIds={[]} locked={false} hasSecretAccess
      onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={vi.fn()} />;
  }
  render(<UiLocaleProvider><Fixture /></UiLocaleProvider>);
  const select = screen.getByLabelText("Provider");
  expect(within(select).getByRole("option", { name: "codex-oauth" })).toHaveValue("original-codex-id");
  expect(within(select).getByRole("option", { name: "chatgpt-web" })).toHaveValue("original-web-id");
  expect(within(select).getByRole("option", { name: "nvidia-api" })).toHaveValue("nvidia-api");
  fireEvent.change(select, { target: { value: "original-web-id" } });
  expect(select).toHaveValue("original-web-id");
  expect(JSON.stringify(document)).toBe(before);
});
