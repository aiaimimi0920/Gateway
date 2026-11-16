import { NeuroProvider } from "@neuro/ui";
import "@testing-library/jest-dom/vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog, type CredentialDialogProps } from "./CredentialDialog";

const base = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const catalog = {
  sourceUrl: base.replace("/compatible-mode/v1", "/api/v1/models"), accountBaseUrl: base,
  checkedAt: "2026-11-02T00:00:00Z", models: ["new-fetched"]
};
function props(): CredentialDialogProps {
  return {
    qwenProvider: () => ({ id: "ordinary", preset: "qwen-official-api", base_url: base }),
    catalogProviderKind: () => "qwen", onDiscoverProviderModels: vi.fn().mockResolvedValue(catalog),
    open: true, mode: "edit", providerOptions: [{ id: "ordinary", label: "ordinary" }], existingCredentialIds: [],
    initialValue: {
      providerId: "ordinary", credentialId: "a", accountName: "", enabled: true, baseUrl: "",
      supportedModelsText: "old-fetched\nmanual", fetchedModelsText: "old-fetched", manualModelsText: "manual", apiKeyOperation: "keep", apiKeyValue: ""
    },
    locked: false, hasSecretAccess: true, onRequestSecretAccess: vi.fn(), onOpenChange: vi.fn(), onSubmit: vi.fn()
  };
}
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
it("refreshes the exact stored account without sending a Key or draft URL, and preserves its native catalog on save", async () => {
  const user = userEvent.setup(); const options = props();
  render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} /></UiLocaleProvider></NeuroProvider>);
  await user.click(screen.getByRole("button", { name: "拉取模型" }));
  await waitFor(() => expect(screen.getByLabelText("拉取模型列表")).toHaveValue("new-fetched"));
  expect(options.onDiscoverProviderModels).toHaveBeenCalledWith({ credentialId: "a" }, expect.any(AbortSignal));
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(options.onSubmit).toHaveBeenCalledWith(expect.objectContaining({
    apiKeyOperation: "keep", baseUrl: "", modelCatalog: catalog,
    fetchedModelsText: "new-fetched", manualModelsText: "manual"
  }));
});

it("binds a newly fetched directory to the explicit replacement Key and target instead of discarding it on submit", async () => {
  const user = userEvent.setup(); const options = props();
  render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} /></UiLocaleProvider></NeuroProvider>);
  await user.type(screen.getByLabelText("API Key"), "synthetic-new-key");
  expect(screen.getByLabelText("拉取模型列表")).toHaveValue("");
  await user.click(screen.getByRole("button", { name: "拉取模型" }));
  await waitFor(() => expect(screen.getByLabelText("拉取模型列表")).toHaveValue("new-fetched"));
  expect(options.onDiscoverProviderModels).toHaveBeenCalledWith({ providerId: "ordinary", apiKey: "synthetic-new-key", accountBaseUrl: base }, expect.any(AbortSignal));
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(options.onSubmit).toHaveBeenCalledWith(expect.objectContaining({
    apiKeyOperation: "replace", modelCatalog: catalog,
    fetchedModelsText: "new-fetched", manualModelsText: "manual"
  }));
  await user.type(screen.getByLabelText("API Key"), "-different");
  expect(screen.getByLabelText("拉取模型列表")).toHaveValue("");
});

it("refuses a stored-key retarget before discovery", async () => {
  const user = userEvent.setup(); const options = props();
  render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} /></UiLocaleProvider></NeuroProvider>);
  await user.selectOptions(screen.getByLabelText("Qwen 目标来源"), "override");
  await user.clear(screen.getByLabelText("Qwen 工作空间")); await user.type(screen.getByLabelText("Qwen 工作空间"), "workspace-b");
  await user.click(screen.getByRole("button", { name: "拉取模型" }));
  expect(options.onDiscoverProviderModels).not.toHaveBeenCalled();
  expect(screen.getByRole("alert")).toHaveTextContent("目标已变化");
});

it("discards late results after permission revocation", async () => {
  const user = userEvent.setup(); const options = props(); let resolve!: (value: typeof catalog) => void;
  options.onDiscoverProviderModels = vi.fn(() => new Promise<typeof catalog>((done) => { resolve = done; }));
  const view = render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} /></UiLocaleProvider></NeuroProvider>);
  await user.click(screen.getByRole("button", { name: "拉取模型" }));
  view.rerender(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} hasSecretAccess={false} /></UiLocaleProvider></NeuroProvider>);
  await act(async () => resolve(catalog));
  expect(screen.getByLabelText("拉取模型列表")).not.toHaveValue("new-fetched");
  expect(options.onSubmit).not.toHaveBeenCalled();
});
it("rejects a catalog for another account target without overwriting manual or old fetched models", async () => {
  const user = userEvent.setup(); const options = props();
  options.onDiscoverProviderModels = vi.fn().mockResolvedValue({
    ...catalog, accountBaseUrl: base.replace("workspace-a", "workspace-b"),
    sourceUrl: catalog.sourceUrl.replace("workspace-a", "workspace-b")
  });
  render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} /></UiLocaleProvider></NeuroProvider>);
  await user.click(screen.getByRole("button", { name: "拉取模型" }));
  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("账号目标不匹配"));
  expect(screen.getByLabelText("拉取模型列表")).toHaveValue("old-fetched");
  expect(screen.getByLabelText("手动模型列表")).toHaveValue("manual");
});
it.each(["close", "parent"])("aborts and ignores late native results after %s changes", async (change) => {
  const user = userEvent.setup(); const options = props(); let resolve!: (value: typeof catalog) => void; let signal!: AbortSignal;
  options.onDiscoverProviderModels = vi.fn((_, requestSignal) => { signal = requestSignal!; return new Promise<typeof catalog>((done) => { resolve = done; }); });
  const view = render(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} /></UiLocaleProvider></NeuroProvider>);
  await user.click(screen.getByRole("button", { name: "拉取模型" }));
  if (change === "close") view.rerender(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} open={false} /></UiLocaleProvider></NeuroProvider>);
  else view.rerender(<NeuroProvider><UiLocaleProvider><CredentialDialog {...options} qwenProvider={() => ({ ...options.qwenProvider!("ordinary"), base_url: base.replace("workspace-a", "workspace-b") })} /></UiLocaleProvider></NeuroProvider>);
  expect(signal.aborted).toBe(true);
  await act(async () => resolve(catalog));
  expect(options.onSubmit).not.toHaveBeenCalled();
  if (change === "parent") expect(screen.getByLabelText("拉取模型列表")).toHaveValue("old-fetched");
});
