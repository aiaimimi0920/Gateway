import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { sapBase } from "./sapAiCoreTestFixtures";

it("enables Bedrock image input only for the explicitly selected deployment and resource group", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "sap-aicore-chat-api" }));
  const mode = screen.getByRole("checkbox", { name: "使用 Bedrock Converse" });
  const image = () => screen.getByRole("checkbox", { name: "启用 Bedrock 图片输入" });
  await user.click(mode);
  expect(image()).toBeDisabled();
  const base = screen.getByLabelText("Base URL");
  const group = screen.getByLabelText("AI-Resource-Group");
  fireEvent.change(base, { target: { value: sapBase } });
  fireEvent.change(group, { target: { value: "initial" } });
  expect(image()).not.toBeChecked();
  await user.click(image());
  fireEvent.change(group, { target: { value: "new-group" } });
  expect(image()).not.toBeChecked();
  await user.click(image());
  fireEvent.change(base, { target: { value: sapBase.replace("deployment1", "deployment2") } });
  expect(image()).not.toBeChecked();
  await user.click(image());
  await user.click(mode);
  expect(screen.queryByRole("checkbox", { name: "启用 Bedrock 图片输入" })).not.toBeInTheDocument();
  await user.click(mode);
  expect(image()).not.toBeChecked();
  await user.click(image());
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledTimes(1);
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "sap-aicore-chat")!;
  const saved = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, onSubmit.mock.calls[0][0]);
  expect(saved.providers[0]).toMatchObject({ supported_models: ["deployment2"], headers: { "AI-Resource-Group": "new-group" },
    credentials: [], extra_body: { sap_bedrock_converse: true, sap_bedrock_image_input: true } });
});
