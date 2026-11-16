import { NeuroProvider } from "@neuro/ui";
import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog, type CredentialDialogProps } from "./CredentialDialog";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
it("keeps inherited Qwen targets inherited and refuses retargeting a kept key", async () => {
  const user = userEvent.setup(); const submit = vi.fn();
  const qwen = {
    qwenProvider: () => ({
      id: "renamed", preset: "qwen-official-api",
      base_url: "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1"
    })
  };
  render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...qwen} open mode="edit" providerOptions={[{ id: "renamed", label: "renamed" }]}
    existingCredentialIds={[]} initialValue={{
      providerId: "renamed", credentialId: "a", accountName: "", enabled: true,
      baseUrl: "", supportedModelsText: "fixture-model", apiKeyOperation: "keep", apiKeyValue: ""
    }}
    locked={false} hasSecretAccess onRequestSecretAccess={vi.fn()} onOpenChange={vi.fn()} onSubmit={submit} /></UiLocaleProvider></NeuroProvider>);
  expect(screen.getByLabelText("Qwen 目标来源")).toHaveValue("inherit");
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(submit).toHaveBeenLastCalledWith(expect.objectContaining({ baseUrl: "", apiKeyOperation: "keep" }));
  submit.mockClear();
  await user.selectOptions(screen.getByLabelText("Qwen 目标来源"), "override");
  await user.selectOptions(screen.getByLabelText("Qwen 地域"), "eu-central-1");
  await user.clear(screen.getByLabelText("Qwen 工作空间"));
  await user.type(screen.getByLabelText("Qwen 工作空间"), "workspace-b");
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(submit).not.toHaveBeenCalled();
  expect(screen.getByRole("alert")).toHaveTextContent("目标已变化");
});

function props(): CredentialDialogProps {
  return {
    qwenProvider: () => ({ id: "renamed", preset: "qwen-official-api", base_url: "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1" }),
    open: true, mode: "edit", providerOptions: [{ id: "renamed", label: "renamed" }], existingCredentialIds: [],
    initialValue: {
      providerId: "renamed", credentialId: "a", accountName: "", enabled: true, baseUrl: "", supportedModelsText: "old-fetched\nmanual",
      fetchedModelsText: "old-fetched", manualModelsText: "manual", apiKeyOperation: "keep", apiKeyValue: ""
    },
    locked: false, hasSecretAccess: true, onRequestSecretAccess: vi.fn(), onOpenChange: vi.fn(), onSubmit: vi.fn()
  };
}

it("can roundtrip override and inheritance without expanding an inherited target or leaking edit state across reopen", async () => {
  const user = userEvent.setup(); const options = props();
  const view = render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} /></UiLocaleProvider></NeuroProvider>);
  await user.selectOptions(screen.getByLabelText("Qwen 目标来源"), "override");
  expect(screen.getByLabelText("Qwen 地域")).toHaveValue("cn-beijing");
  expect(screen.getByLabelText("拉取模型列表")).toHaveValue("old-fetched");
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(options.onSubmit).toHaveBeenLastCalledWith(expect.objectContaining({ baseUrl: "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1", apiKeyOperation: "keep" }));
  await user.selectOptions(screen.getByLabelText("Qwen 目标来源"), "inherit");
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(options.onSubmit).toHaveBeenLastCalledWith(expect.objectContaining({ baseUrl: "", apiKeyOperation: "keep", qwenEndpoint: undefined }));
  await user.selectOptions(screen.getByLabelText("Qwen 目标来源"), "override");
  await user.clear(screen.getByLabelText("Qwen 工作空间"));
  view.rerender(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} open={false} /></UiLocaleProvider></NeuroProvider>);
  view.rerender(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} /></UiLocaleProvider></NeuroProvider>);
  expect(screen.getByLabelText("Qwen 目标来源")).toHaveValue("inherit");
  expect(screen.getByLabelText("拉取模型列表")).toHaveValue("old-fetched");
});

it("requires complete target input and permits explicit replace or clear while revoking stale fetched models", async () => {
  const user = userEvent.setup(); const options = props();
  render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} /></UiLocaleProvider></NeuroProvider>);
  await user.selectOptions(screen.getByLabelText("Qwen 目标来源"), "override");
  await user.clear(screen.getByLabelText("Qwen 工作空间"));
  await user.type(screen.getByLabelText("API Key"), "synthetic-new-key");
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(options.onSubmit).not.toHaveBeenCalled();
  expect(screen.getByRole("alert")).toHaveTextContent("有效工作空间");
  await user.type(screen.getByLabelText("Qwen 工作空间"), "workspace-b");
  await user.selectOptions(screen.getByLabelText("Qwen 地域"), "cn-hongkong");
  expect(screen.getByLabelText("拉取模型列表")).toHaveValue("");
  expect(screen.getByLabelText("手动模型列表")).toHaveValue("manual");
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(options.onSubmit).toHaveBeenLastCalledWith(expect.objectContaining({
    apiKeyOperation: "replace", apiKeyValue: "synthetic-new-key",
    supportedModelsText: "manual", baseUrl: "https://workspace-b.cn-hongkong.maas.aliyuncs.com/compatible-mode/v1"
  }));
  await user.clear(screen.getByLabelText("API Key"));
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(options.onSubmit).toHaveBeenLastCalledWith(expect.objectContaining({ apiKeyOperation: "clear", apiKeyValue: "", supportedModelsText: "manual" }));
});

it("does not invoke generic discovery or another official catalog for the ordinary Qwen product", async () => {
  const user = userEvent.setup(); const options = props();
  const discover = vi.fn(); const catalog = vi.fn();
  render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} onDiscover={discover} discoveryProvider={() => ({ baseUrl: "https://wrong.invalid" })}
    catalogProviderKind={() => "nvidia"} onDiscoverProviderModels={catalog} /></UiLocaleProvider></NeuroProvider>);
  expect(screen.queryByRole("button", { name: "刷新协议" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "拉取模型" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(options.onSubmit).toHaveBeenCalledTimes(1);
  expect(discover).not.toHaveBeenCalled(); expect(catalog).not.toHaveBeenCalled();
});
