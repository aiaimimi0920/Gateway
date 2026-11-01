import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { GoogleWebAccountImport, googleWebAuthFamily } from "./GoogleWebAccountImport";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
it.each([
  ["gemini-web-chat", "gemini-web", "Gemini Web"],
  ["gemini-web-chat-modular", "gemini-web", "Gemini Web"],
  ["gemini-canvas-browser-relay", "gemini-canvas", "Gemini Canvas"],
  ["gemini-canvas", "gemini-canvas", "Gemini Canvas"],
  ["gemini-canvas-program-relay", "gemini-canvas", "Gemini Canvas"],
  ["gemini-canvas-chat", "gemini-canvas-chat", "Gemini Canvas"],
  ["aistudio-web-reverse", "aistudio-web", "AI Studio"],
  ["flow-web", "flow-web", "Google Flow"],
  ["stitch-web", "stitch-web", "Google Stitch"],
])("routes %s to its own dedicated browser import", (preset, family, label) => {
  const start = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "renamed-pool", label: "Pool" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onSubmit={vi.fn()} onOpenChange={vi.fn()}
    onRequestSecretAccess={vi.fn()} renderAuthentication={() => <GoogleWebAccountImport preset={preset}
      providerId="renamed-pool" disabled={false} onStart={start} />} /></UiLocaleProvider>);
  expect(googleWebAuthFamily(preset)).toBe(family);
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /^保存$/ })).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("账号名称"), { target: { value: "My account" } });
  fireEvent.click(screen.getByRole("button", { name: `${label} 网页登录并导入` }));
  expect(start).toHaveBeenCalledWith(family, "renamed-pool", "My account");
});

it("does not start imports when disabled and does not treat API presets as web", () => {
  const start = vi.fn();
  render(<UiLocaleProvider><GoogleWebAccountImport preset="aistudio-web-reverse" providerId="pool"
    disabled onStart={start} /></UiLocaleProvider>);
  expect(screen.getByRole("button", { name: "AI Studio 网页登录并导入" })).toBeDisabled();
  expect(googleWebAuthFamily("gemini")).toBeNull();
  expect(googleWebAuthFamily("chatgpt-web-reverse")).toBeNull();
  expect(start).not.toHaveBeenCalled();
});

it("clears the account label when the selected provider changes", () => {
  const start = vi.fn();
  const view = render(<UiLocaleProvider><GoogleWebAccountImport preset="aistudio-web-reverse" providerId="first"
    disabled={false} onStart={start} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号名称"), { target: { value: "Private label" } });
  view.rerender(<UiLocaleProvider><GoogleWebAccountImport preset="gemini-web-chat" providerId="second"
    disabled={false} onStart={start} /></UiLocaleProvider>);
  expect(screen.getByLabelText("账号名称")).toHaveValue("");
});

it.each(["aistudio-web-reverse", "gemini-web-chat", "gemini-canvas-browser-relay", "flow-web", "stitch-web"])("keeps existing %s credentials when editing model choices", (preset) => {
  const submit = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="edit" providerOptions={[{ id: "pool", label: "Pool" }]}
    existingCredentialIds={["existing"]} locked={false} hasSecretAccess onSubmit={submit} onOpenChange={vi.fn()}
    onRequestSecretAccess={vi.fn()} managedWebProvider={(id) => id === "pool"}
    initialValue={{ providerId: "pool", credentialId: "existing", accountName: "Preserved name", enabled: true,
      baseUrl: "", supportedModelsText: "existing-model", apiKeyOperation: "keep", apiKeyValue: "" }} /></UiLocaleProvider>);
  expect(googleWebAuthFamily(preset)).not.toBeNull();
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  expect(screen.getByText("网页登录凭据已保存")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /^保存$/ }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "existing", apiKeyOperation: "keep", apiKeyValue: "" }));
});

it("requires secret access before collecting JSON and clears material on method, grant and provider changes", () => {
  const imported = vi.fn(), requestAccess = vi.fn();
  const props = { preset: "aistudio-web-reverse", providerId: "first", disabled: false,
    onStart: vi.fn(), onImportJson: imported, onRequestSecretAccess: requestAccess };
  const view = render(<UiLocaleProvider><GoogleWebAccountImport {...props} hasSecretAccess={false} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "json" } });
  expect(screen.queryByLabelText("网页会话 JSON")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "确认敏感信息访问权限" })); expect(requestAccess).toHaveBeenCalledOnce();
  view.rerender(<UiLocaleProvider><GoogleWebAccountImport {...props} hasSecretAccess /></UiLocaleProvider>);
  for (const change of ["method", "grant", "provider"]) {
    fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { value: "private-material" } });
    if (change === "method") {
      fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "browser" } });
      fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "json" } });
    } else if (change === "grant") {
      view.rerender(<UiLocaleProvider><GoogleWebAccountImport {...props} hasSecretAccess={false} /></UiLocaleProvider>);
      view.rerender(<UiLocaleProvider><GoogleWebAccountImport {...props} hasSecretAccess /></UiLocaleProvider>);
    } else {
      view.rerender(<UiLocaleProvider><GoogleWebAccountImport {...props} providerId="second" hasSecretAccess /></UiLocaleProvider>);
      fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "json" } });
    }
    expect(screen.getByLabelText("网页会话 JSON")).toHaveValue("");
  }
  expect(imported).not.toHaveBeenCalled();
});
