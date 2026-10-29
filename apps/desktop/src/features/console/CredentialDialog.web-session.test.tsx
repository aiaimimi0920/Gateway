import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog, type CredentialDialogProps } from "./CredentialDialog";
import type { WebBrowserCapture } from "./ChatgptWebBrowserImport";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
const raw = JSON.stringify({ authToken: "web-fixture", expiresAt: "2099-01-01T00:00:00Z",
  cookieHeader: "session=fixture", userAgent: "Browser/1", deviceId: "device", sessionId: "session" });

function setup(overrides: Partial<CredentialDialogProps> = {}) {
  const props: CredentialDialogProps = {
    open: true, mode: "add", providerOptions: [{ id: "web", label: "chatgpt-web" }, { id: "codex", label: "codex-oauth" }],
    existingCredentialIds: [], initialValue: { providerId: "web", credentialId: "web-account", accountName: "", enabled: true,
      baseUrl: "", supportedModelsText: "gpt-5.4", apiKeyOperation: "replace", apiKeyValue: "" },
    locked: false, hasSecretAccess: true, onOpenChange: vi.fn(), onRequestSecretAccess: vi.fn(), onSubmit: vi.fn(),
    supportsWebSession: (id) => id === "web", renderAuthentication: vi.fn(() => <span>OAuth panel</span>),
    onDiscoverChatgptModels: vi.fn(), onDiscoverProviderModels: vi.fn(), ...overrides,
  };
  const view = render(<UiLocaleProvider><CredentialDialog {...props} /></UiLocaleProvider>);
  return { props, view };
}

it("uses an independent file import, never OAuth or Codex model discovery", async () => {
  const { props } = setup();
  expect(screen.queryByText("OAuth panel")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).not.toHaveBeenCalled();
  const file = { size: raw.length, text: () => Promise.resolve(raw) };
  fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { files: [file] } });
  await screen.findByText("网页会话材料已读取，待保存");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyValue: "web-fixture",
    webSessionMaterial: expect.objectContaining({ cookieHeader: "session=fixture" }) }));
  expect(props.renderAuthentication).not.toHaveBeenCalled();
  expect(props.onDiscoverChatgptModels).not.toHaveBeenCalled();
  expect(props.onDiscoverProviderModels).not.toHaveBeenCalled();
});

it("requires a secret grant before reading a file", () => {
  setup({ hasSecretAccess: false });
  expect(screen.getByLabelText("网页会话 JSON")).toBeDisabled();
});

it("does not save the previous material after a replacement file is rejected", async () => {
  const { props } = setup();
  fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { files: [{ size: raw.length, text: () => Promise.resolve(raw) }] } });
  await screen.findByText("网页会话材料已读取，待保存");
  fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { files: [{ size: 1, text: () => Promise.resolve("{") }] } });
  await screen.findByText("网页会话文件不是有效的 JSON。");
  expect(screen.queryByText("网页会话材料已读取，待保存")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).not.toHaveBeenCalled();
});

it("discards a pending file read when the provider changes", async () => {
  let finish!: (value: string) => void;
  setup();
  fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { files: [{ size: 100, text: () => new Promise<string>((resolve) => { finish = resolve; }) }] } });
  fireEvent.change(screen.getByLabelText("Provider"), { target: { value: "codex" } });
  await act(async () => { finish(raw); });
  expect(screen.getByText("OAuth panel")).toBeInTheDocument();
  expect(screen.queryByText("网页会话材料已读取，待保存")).not.toBeInTheDocument();
});

it("clears loaded material on secret grant revocation", async () => {
  const { props, view } = setup();
  fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { files: [{ size: raw.length, text: () => Promise.resolve(raw) }] } });
  await screen.findByText("网页会话材料已读取，待保存");
  view.rerender(<UiLocaleProvider><CredentialDialog {...props} hasSecretAccess={false} /></UiLocaleProvider>);
  await waitFor(() => expect(screen.queryByText("网页会话材料已读取，待保存")).not.toBeInTheDocument());
});

it("imports native-browser material through the Web owner, never OAuth or model discovery", async () => {
  const capture = vi.fn<WebBrowserCapture>().mockResolvedValue({ material: JSON.parse(raw), accountName: "Web account" });
  const { props } = setup({ onCaptureWebBrowser: capture });
  fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
  expect(screen.queryByLabelText("浏览器调试端口")).not.toBeInTheDocument();
  await screen.findByText("已读取账号：Web account");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyValue: "web-fixture", accountName: "Web account" }));
  expect(capture).toHaveBeenCalledWith(expect.objectContaining({ operation: "login", providerId: "web" }), expect.any(AbortSignal));
  expect(props.renderAuthentication).not.toHaveBeenCalled();
  expect(props.onDiscoverChatgptModels).not.toHaveBeenCalled();
  expect(props.onDiscoverProviderModels).not.toHaveBeenCalled();
});

it("disables native login without a secret grant and never asks to install an extension", () => {
  setup({ onCaptureWebBrowser: vi.fn<WebBrowserCapture>(), hasSecretAccess: false });
  expect(screen.getByRole("button", { name: "网页登录" })).toBeDisabled();
  expect(screen.queryByText(/会话导出扩展/)).not.toBeInTheDocument();
});

it("preserves captured material across provider-list and reveal-callback refreshes", async () => {
  const capture = vi.fn<WebBrowserCapture>().mockResolvedValue({ material: JSON.parse(raw), accountName: "Web account" });
  const { props, view } = setup({ onCaptureWebBrowser: capture });
  fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
  await screen.findByText("已读取账号：Web account");
  view.rerender(<UiLocaleProvider><CredentialDialog {...props}
    providerOptions={props.providerOptions.map((option) => ({ ...option }))} onRevealKey={vi.fn()} /></UiLocaleProvider>);
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyValue: "web-fixture",
    webSessionMaterial: expect.objectContaining({ cookieHeader: "session=fixture" }) }));
});

it("keeps native capture alive when editing the local account ID", async () => {
  let finish!: (result: Awaited<ReturnType<WebBrowserCapture>>) => void;
  const capture = vi.fn<WebBrowserCapture>((input) => input.operation === "cancel" ? Promise.resolve({})
    : new Promise((resolve) => { finish = resolve; }));
  const { props } = setup({ onCaptureWebBrowser: capture });
  fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
  const signal = capture.mock.calls[0][1];
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "renamed-account" } });
  expect(signal.aborted).toBe(false);
  await act(async () => { finish({ material: JSON.parse(raw), accountName: "Web account" }); });
  await screen.findByText("已读取账号：Web account");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "renamed-account", apiKeyValue: "web-fixture" }));
});

it("clears the captured session when closing and reopening the dialog", async () => {
  const capture = vi.fn<WebBrowserCapture>().mockResolvedValue({ material: JSON.parse(raw), accountName: "Web account" });
  const { props, view } = setup({ onCaptureWebBrowser: capture });
  fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
  await screen.findByText("已读取账号：Web account");
  view.rerender(<UiLocaleProvider><CredentialDialog {...props} open={false} /></UiLocaleProvider>);
  view.rerender(<UiLocaleProvider><CredentialDialog {...props} /></UiLocaleProvider>);
  expect(screen.queryByText("已读取账号：Web account")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).not.toHaveBeenCalled();
});

it("cancels pending capture when the editing target changes", async () => {
  const capture = vi.fn<WebBrowserCapture>((input) => input.operation === "cancel" ? Promise.resolve({}) : new Promise(() => {}));
  const { props, view } = setup({ onCaptureWebBrowser: capture });
  fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
  const signal = capture.mock.calls[0][1];
  view.rerender(<UiLocaleProvider><CredentialDialog {...props}
    initialValue={{ ...props.initialValue!, credentialId: "other-account" }} /></UiLocaleProvider>);
  expect(signal.aborted).toBe(true);
  expect(screen.getByLabelText("账号 ID")).toHaveValue("other-account");
});

it("switching to JSON cancels browser capture and rejects its late result", async () => {
  let finish!: (result: Awaited<ReturnType<WebBrowserCapture>>) => void;
  const capture = vi.fn<WebBrowserCapture>((input) => input.operation === "cancel" ? Promise.resolve({})
    : new Promise((resolve) => { finish = resolve; }));
  const { props } = setup({ onCaptureWebBrowser: capture });
  expect(screen.queryByLabelText("网页会话 JSON")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
  const signal = capture.mock.calls[0][1];
  fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "json" } });
  expect(signal.aborted).toBe(true);
  expect(capture.mock.calls[1][0]).toEqual({ operation: "cancel", providerId: "web", requestId: capture.mock.calls[0][0].requestId });
  fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { files: [{ size: raw.length, text: () => Promise.resolve(raw) }] } });
  await screen.findByText("网页会话材料已读取，待保存");
  await act(async () => { finish({ material: { ...JSON.parse(raw), authToken: "stale-browser-token" } }); });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyValue: "web-fixture" }));
});

it("switching to browser rejects a late file read and clears already loaded material", async () => {
  let finish!: (value: string) => void;
  const { props } = setup({ onCaptureWebBrowser: vi.fn<WebBrowserCapture>() });
  fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "json" } });
  fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { files: [{ size: raw.length, text: () => Promise.resolve(raw) }] } });
  await screen.findByText("网页会话材料已读取，待保存");
  fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "browser" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "json" } });
  fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { files: [{ size: raw.length,
    text: () => new Promise<string>((resolve) => { finish = resolve; }) }] } });
  fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "browser" } });
  await act(async () => { finish(raw); });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).not.toHaveBeenCalled();
});
