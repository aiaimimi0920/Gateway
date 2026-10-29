import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog, type CredentialDialogProps } from "./CredentialDialog";

const catalog = { sourceUrl: "https://api.poe.com/v1/models" as const,
  checkedAt: "2026-10-26T15:00:00Z", models: ["shared", "fetched"] };
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
function setup(overrides: Partial<CredentialDialogProps> = {}) {
  const props: CredentialDialogProps = { open: true, mode: "add", providerOptions: [{ id: "poe", label: "Poe" }],
    existingCredentialIds: [], locked: false, hasSecretAccess: true, onOpenChange: vi.fn(),
    onRequestSecretAccess: vi.fn(), onSubmit: vi.fn(), catalogProviderKind: () => "poe",
    onDiscoverProviderModels: vi.fn().mockResolvedValue(catalog), ...overrides };
  const view = render(<UiLocaleProvider><CredentialDialog {...props} /></UiLocaleProvider>);
  return { props, view };
}
function enterAccount() {
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
}

it("shows one brand label and removes redundant account fields", () => {
  setup();
  expect(screen.getByRole("option", { name: "Poe" })).toHaveValue("poe");
  expect(screen.queryByLabelText("账号名称")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("账号覆盖地址")).not.toBeInTheDocument();
  expect(screen.getByLabelText("拉取模型列表")).toBeInTheDocument();
  expect(screen.getByLabelText("手动模型列表")).toBeInTheDocument();
});

it("fetches into the left editor without overwriting manual models and saves their unique union", async () => {
  const { props } = setup(); enterAccount();
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "manual, shared\nmanual" } });
  fireEvent.click(screen.getByRole("button", { name: "拉取模型" }));
  await waitFor(() => expect(screen.getByLabelText("拉取模型列表")).toHaveValue("shared\nfetched"));
  expect(screen.getByLabelText("手动模型列表")).toHaveValue("manual, shared\nmanual");
  expect(props.onSubmit).not.toHaveBeenCalled();
  expect(props.onDiscoverProviderModels).toHaveBeenCalledWith({ providerId: "poe", apiKey: "fixture-key" }, expect.any(AbortSignal));
  fireEvent.change(screen.getByLabelText("拉取模型列表"), { target: { value: "fetched" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({
    supportedModelsText: "manual\nshared\nfetched", fetchedModelsText: "fetched", manualModelsText: "manual, shared\nmanual",
  }));
  expect(props.onDiscoverProviderModels).toHaveBeenCalledTimes(1);
});

it("keeps catalog feedback in the same reserved slot below the fetch button", async () => {
  let resolve!: (value: typeof catalog) => void;
  setup({ onDiscoverProviderModels: vi.fn(() => new Promise<typeof catalog>((done) => { resolve = done; })) });
  enterAccount();
  const button = screen.getByRole("button", { name: "拉取模型" });
  const status = screen.getByRole("status");
  const form = button.closest("form")!;
  const formChildren = form.childElementCount;
  expect(button.nextElementSibling).toBe(status);
  expect(status).toHaveClass("nt-credential-model-status");
  expect(status).toBeEmptyDOMElement();
  fireEvent.click(button);
  expect(screen.getByRole("status")).toBe(status);
  await act(async () => resolve(catalog));
  expect(status).toHaveTextContent("目录已发现 2");
  expect(screen.getByRole("status")).toBe(status);
  expect(form.childElementCount).toBe(formChildren);
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "changed-fixture-key" } });
  expect(screen.getByRole("status")).toBe(status);
  expect(status).toBeEmptyDOMElement();
});

it("allows manual-only saving after a directory failure without retrying the upstream", async () => {
  const { props } = setup({ onDiscoverProviderModels: vi.fn().mockRejectedValue(new Error("catalog offline")) });
  enterAccount();
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "manual-only" } });
  fireEvent.click(screen.getByRole("button", { name: "拉取模型" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("catalog offline");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ supportedModelsText: "manual-only" }));
  expect(props.onDiscoverProviderModels).toHaveBeenCalledTimes(1);
});

it("keeps hidden legacy name and endpoint unchanged while editing model sources", () => {
  const { props } = setup({ mode: "edit", initialValue: { providerId: "poe", credentialId: "existing",
    accountName: "Legacy", baseUrl: "https://api.poe.com/v1", supportedModelsText: "shared\nmanual",
    modelCatalog: catalog, enabled: false, apiKeyOperation: "keep", apiKeyValue: "" } });
  expect(screen.getByLabelText("拉取模型列表")).toHaveValue("shared");
  expect(screen.getByLabelText("手动模型列表")).toHaveValue("manual");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({
    accountName: "Legacy", baseUrl: "https://api.poe.com/v1", enabled: false, apiKeyOperation: "keep",
  }));
});

it("never treats generic protocol discovery as a directory-only fetch", () => {
  const onDiscover = vi.fn();
  setup({ catalogProviderKind: () => undefined, discoveryProvider: () => ({ baseUrl: "https://fixture.test" }), onDiscover });
  expect(screen.getByRole("button", { name: "拉取模型" })).toBeDisabled();
  expect(screen.getByLabelText("手动模型列表")).toBeEnabled();
  expect(onDiscover).not.toHaveBeenCalled();
});

it("fences late directory responses after closing and clears sources on reopen", async () => {
  let resolve!: (value: typeof catalog) => void;
  const { props, view } = setup({ providerOptions: [{ id: "poe", label: "Poe" }, { id: "other", label: "Other" }],
    onDiscoverProviderModels: vi.fn(() => new Promise<typeof catalog>((done) => { resolve = done; })) });
  fireEvent.change(screen.getByLabelText("Provider"), { target: { value: "poe" } });
  enterAccount();
  fireEvent.click(screen.getByRole("button", { name: "拉取模型" }));
  view.rerender(<UiLocaleProvider><CredentialDialog {...props} open={false} /></UiLocaleProvider>);
  await act(async () => resolve(catalog));
  expect(props.onSubmit).not.toHaveBeenCalled();
  view.rerender(<UiLocaleProvider><CredentialDialog {...props} /></UiLocaleProvider>);
  expect(screen.getByLabelText("拉取模型列表")).toHaveValue("");
  expect(screen.getByLabelText("手动模型列表")).toHaveValue("");
});

it("clears model sources and hidden endpoint overrides when switching the provider", () => {
  setup({ providerOptions: [{ id: "poe", label: "Poe" }, { id: "other", label: "Other" }],
    initialValue: { providerId: "poe", credentialId: "copy", accountName: "Legacy", baseUrl: "https://legacy.test",
      supportedModelsText: "manual", enabled: true, apiKeyOperation: "replace", apiKeyValue: "fixture-key" } });
  fireEvent.change(screen.getByLabelText("Provider"), { target: { value: "other" } });
  expect(screen.getByLabelText("拉取模型列表")).toHaveValue("");
  expect(screen.getByLabelText("手动模型列表")).toHaveValue("");
  expect(screen.getByLabelText("API Key")).toHaveValue("");
});

it("does not refetch or inherit models after intentionally clearing an existing selection", () => {
  const { props } = setup({ mode: "edit", initialValue: { providerId: "poe", credentialId: "existing", accountName: "",
    baseUrl: "", supportedModelsText: "old", fetchedModelsText: "old", manualModelsText: "", enabled: true,
    apiKeyOperation: "keep", apiKeyValue: "" } });
  fireEvent.change(screen.getByLabelText("拉取模型列表"), { target: { value: "" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ supportedModelsText: "", fetchedModelsText: "", manualModelsText: "" }));
  expect(props.onDiscoverProviderModels).not.toHaveBeenCalled();
});

it("rejects invalid exact model IDs before saving", () => {
  const { props } = setup(); enterAccount();
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "gpt-*" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(screen.getByRole("alert")).toHaveTextContent("模型 ID 无效");
  expect(props.onSubmit).not.toHaveBeenCalled();
});

it("aborts a pending official catalog when secret access is revoked", async () => {
  let resolve!: (value: typeof catalog) => void;
  const { props, view } = setup({ onDiscoverProviderModels: vi.fn(() => new Promise<typeof catalog>((done) => { resolve = done; })) });
  enterAccount(); fireEvent.click(screen.getByRole("button", { name: "拉取模型" }));
  const signal = vi.mocked(props.onDiscoverProviderModels!).mock.calls[0][1]!;
  view.rerender(<UiLocaleProvider><CredentialDialog {...props} hasSecretAccess={false} /></UiLocaleProvider>);
  await act(async () => resolve(catalog));
  expect(signal.aborted).toBe(true);
  expect(screen.getByLabelText("拉取模型列表")).toHaveValue("");
  expect(props.onSubmit).not.toHaveBeenCalled();
});
