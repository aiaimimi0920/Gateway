import { NeuroProvider } from "@neuro/ui";
import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog, type CredentialDialogProps } from "./CredentialDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { routeDocument } from "./credentialDocumentTestFixtures";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
const provider = { id: "renamed", preset: "bedrock-converse", base_url: "https://bedrock-runtime.us-east-1.amazonaws.com" };
function setup(overrides: Partial<CredentialDialogProps> = {}) {
  const props = {
    open: true, mode: "add" as const, providerOptions: [{ id: "renamed", label: "Bedrock" }, { id: "api", label: "API" }],
    existingCredentialIds: [], initialValue: {
      providerId: "renamed", credentialId: "a", accountName: "", enabled: true,
      baseUrl: "", supportedModelsText: "fixture-model", apiKeyOperation: "replace" as const, apiKeyValue: ""
    },
    locked: false, hasSecretAccess: true, onOpenChange: vi.fn(), onRequestSecretAccess: vi.fn(), onSubmit: vi.fn(),
    bedrockProvider: (id: string) => id === "renamed" ? provider : undefined, ...overrides
  };
  const view = render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...props} /></UiLocaleProvider></NeuroProvider>);
  return { props, view };
}
function iam() {
  fireEvent.change(screen.getByLabelText("Bedrock 认证模式"), { target: { value: "aws_sigv4_temporary" } });
  fireEvent.change(screen.getByLabelText("AWS Access Key ID"), { target: { value: "synthetic-access" } });
  fireEvent.change(screen.getByLabelText("AWS Secret Access Key"), { target: { value: "synthetic-secret" } });
}
it("offers an empty native Bedrock template without invented models", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find((t) => t.id === "bedrock-api")!;
  expect(template).toMatchObject({ preset: "bedrock-converse", supportedModels: [] });
  const document = addCatalogProvider(routeDocument([]), template, {
    templateId: template.id, providerId: "renamed", providerLabel: "Bedrock",
    vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl: template.baseUrl, supportedModels: [], workflow: defaultCatalogWorkflow(template)
  });
  expect(document.providers[0]).toMatchObject({ explicit_credentials_only: true, credentials: [], preset: "bedrock-converse", supported_models: [], call_flow: { call_adapter: "auto", special_adapter: "auto" } });
  expect(document.model_routes).toEqual([]);
});
it("requires complete temporary IAM material and never asks for a generic API Key", () => {
  const { props } = setup();
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  iam();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("AWS Session Token"), { target: { value: "synthetic-sts" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ bedrockMaterial: expect.objectContaining({ authMode: "aws_sigv4_temporary", sessionToken: "synthetic-sts" }) }));
});
it("clears all typed material on mode, region and provider changes", () => {
  const { props } = setup(); iam();
  fireEvent.change(screen.getByLabelText("AWS 区域"), { target: { value: "us-west-2" } });
  expect(screen.getByLabelText("AWS Access Key ID")).toHaveValue("");
  fireEvent.change(screen.getByLabelText("AWS Access Key ID"), { target: { value: "synthetic-access" } });
  fireEvent.change(screen.getByLabelText("Bedrock 认证模式"), { target: { value: "bedrock_bearer" } });
  expect(screen.getByLabelText("Bedrock API Key")).toHaveValue("");
  fireEvent.change(screen.getByLabelText("Provider"), { target: { value: "api" } });
  expect(screen.getByLabelText("API Key")).toHaveValue("");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).not.toHaveBeenCalled();
});
it("drops secrets on permission revocation, while metadata-only existing edits keep", async () => {
  const { props, view } = setup(); iam();
  view.rerender(<NeuroProvider><UiLocaleProvider><CredentialDialog {...props} hasSecretAccess={false} /></UiLocaleProvider></NeuroProvider>);
  await waitFor(() => expect(screen.getByLabelText("AWS Access Key ID")).toHaveValue(""));
  expect(screen.getByLabelText("AWS Secret Access Key")).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onRequestSecretAccess).toHaveBeenCalled();
});
it("does not convert a legacy account when saving only its model metadata", () => {
  const { props } = setup({
    mode: "edit", hasSecretAccess: false,
    initialValue: {
      providerId: "renamed", credentialId: "a", accountName: "", enabled: true, baseUrl: "",
      supportedModelsText: "fixture-model", apiKeyOperation: "keep", apiKeyValue: ""
    }
  });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyOperation: "keep" }));
});
it("does not expose generic discovery even if a caller also supplies an API classifier", () => {
  const discover = vi.fn();
  setup({ catalogProviderKind: () => "openai", onDiscoverProviderModels: discover, discoveryProvider: () => ({ baseUrl: "https://example.test" }), onDiscover: discover });
  expect(screen.getByRole("button", { name: "拉取模型" })).toBeDisabled();
  expect(screen.queryByRole("button", { name: "刷新协议" })).not.toBeInTheDocument();
});
