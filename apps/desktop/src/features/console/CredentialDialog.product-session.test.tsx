import { NeuroProvider } from "@neuro/ui";
import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog, type CredentialDialogProps } from "./CredentialDialog";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
const raw = JSON.stringify({ product: "grok-web", token: "synthetic-session-token" });
function setup(overrides: Partial<CredentialDialogProps> = {}) {
  const props: CredentialDialogProps = {
    open: true, mode: "add", providerOptions: [{ id: "web", label: "Web" }, { id: "api", label: "API" }],
    existingCredentialIds: [], initialValue: {
      providerId: "web", credentialId: "account", accountName: "", enabled: true,
      baseUrl: "", supportedModelsText: "fixture-model", apiKeyOperation: "replace", apiKeyValue: ""
    },
    locked: false, hasSecretAccess: true, onOpenChange: vi.fn(), onRequestSecretAccess: vi.fn(), onSubmit: vi.fn(),
    sessionProduct: (id) => id === "web" ? "grok-web" : undefined, ...overrides
  };
  const view = render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...props} /></UiLocaleProvider></NeuroProvider>);
  return { props, view };
}
function paste() {
  fireEvent.change(screen.getByLabelText("会话材料 JSON"), { target: { value: raw } });
  fireEvent.click(screen.getByRole("button", { name: "读取会话材料" }));
}
it("imports via a product-specific material field, not API key or OAuth", () => {
  const { props } = setup({ renderAuthentication: vi.fn(() => <span>OAuth panel</span>) });
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).not.toHaveBeenCalled();
  paste();
  expect(screen.getByLabelText("会话材料 JSON")).toHaveValue("");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ sessionMaterial: { product: "grok-web", token: "synthetic-session-token" } }));
  expect(props.renderAuthentication).not.toHaveBeenCalled();
});
it("drops loaded data on provider change and does not leak it to the API field", () => {
  const { props } = setup(); paste();
  fireEvent.change(screen.getByLabelText("Provider"), { target: { value: "api" } });
  expect(screen.getByLabelText("API Key")).toHaveValue("");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).not.toHaveBeenCalled();
});
it("rejects a late file result after the provider changes", async () => {
  let finish!: (source: string) => void;
  setup();
  fireEvent.change(screen.getByLabelText("导入会话文件"), { target: { files: [{ size: 100, text: () => new Promise<string>((resolve) => { finish = resolve; }) }] } });
  fireEvent.change(screen.getByLabelText("Provider"), { target: { value: "api" } });
  await act(async () => finish(raw));
  expect(screen.getByLabelText("API Key")).toHaveValue("");
});
it("clears loaded material and unfinished paste when secret access is revoked", async () => {
  const { props, view } = setup(); paste();
  fireEvent.change(screen.getByLabelText("会话材料 JSON"), { target: { value: raw } });
  view.rerender(<NeuroProvider><UiLocaleProvider><CredentialDialog {...props} hasSecretAccess={false} /></UiLocaleProvider></NeuroProvider>);
  await waitFor(() => expect(screen.queryByText("会话材料已读取，待保存")).not.toBeInTheDocument());
  expect(screen.getByLabelText("会话材料 JSON")).toHaveValue("");
  expect(screen.getByLabelText("导入会话文件")).toBeDisabled();
});
it("keeps stored secrets for metadata-only edits without requiring a new import", () => {
  const { props } = setup({
    mode: "edit", initialValue: {
      providerId: "web", credentialId: "account", accountName: "", enabled: true,
      baseUrl: "", supportedModelsText: "fixture-model", apiKeyOperation: "keep", apiKeyValue: ""
    }
  });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyOperation: "keep" }));
});
it("can request secret access for an existing session account while keeping its secrets", () => {
  const { props } = setup({
    mode: "edit", hasSecretAccess: false, initialValue: {
      providerId: "web", credentialId: "account", accountName: "", enabled: true,
      baseUrl: "", supportedModelsText: "fixture-model", apiKeyOperation: "keep", apiKeyValue: ""
    }
  });
  fireEvent.click(screen.getByRole("button", { name: "确认敏感信息访问权限" }));
  expect(props.onRequestSecretAccess).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyOperation: "keep" }));
});
