import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog, type CredentialDialogProps } from "./CredentialDialog";
import type { AccountDiscovery } from "./accountDiscovery";

const discovery: AccountDiscovery = {
  source_url: "https://fixture.test", api_base: "https://fixture.test/v1", protocol: "chat_completions",
  models: ["chat-only", "messages-only", "unverified"], verified_models: ["chat-only"], checked_at: "now", binding: "fixture",
  protocols: [
    { protocol: "chat_completions", api_base: "https://fixture.test/v1", verified_models: ["chat-only"], failed_models: [] },
    { protocol: "messages", api_base: "https://fixture.test/claude/v1", verified_models: ["messages-only"], failed_models: [] },
  ],
};
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
function setup(overrides: Partial<CredentialDialogProps> = {}) {
  const props: CredentialDialogProps = {
    open: true, mode: "edit", providerOptions: [{ id: "pool", label: "Pool" }], existingCredentialIds: ["account"],
    initialValue: { providerId: "pool", credentialId: "account", accountName: "Account", enabled: false,
      baseUrl: "", supportedModelsText: "", apiKeyOperation: "keep", apiKeyValue: "", discovery },
    locked: false, hasSecretAccess: true, onOpenChange: vi.fn(), onRequestSecretAccess: vi.fn(), onSubmit: vi.fn(),
    discoveryProvider: () => ({ baseUrl: discovery.source_url }), onDiscover: vi.fn().mockResolvedValue(discovery),
    onRevealKey: vi.fn().mockResolvedValue("fixture-secret"), ...overrides,
  };
  const view = render(<UiLocaleProvider><CredentialDialog {...props} /></UiLocaleProvider>);
  return { props, view };
}

it("uses compact fields and keeps enabled state without an extra toggle", () => {
  const { props } = setup();
  expect(screen.queryByLabelText("API Key 操作")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("账号启用状态")).not.toBeInTheDocument();
  expect(screen.queryByText(/写入草稿/)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ enabled: false, apiKeyOperation: "keep" }));
});

it("shows only each protocol's verified models in independent disclosures", () => {
  setup();
  const chat = screen.getByText("Chat Completions").closest("details")!;
  const messages = screen.getByText("Anthropic Messages").closest("details")!;
  expect(chat).not.toHaveAttribute("open");
  fireEvent.click(within(chat).getByText("Chat Completions"));
  expect(chat).toHaveAttribute("open");
  expect(messages).not.toHaveAttribute("open");
  expect(within(chat).getByText("chat-only")).toBeInTheDocument();
  expect(within(chat).queryByText("messages-only")).not.toBeInTheDocument();
  expect(screen.queryByText("unverified")).not.toBeInTheDocument();
});

it("reveals the real key without replacing it just by viewing", async () => {
  const { props } = setup();
  fireEvent.click(screen.getByRole("button", { name: "显示 API Key" }));
  await waitFor(() => expect(screen.getByLabelText("API Key")).toHaveValue("fixture-secret"));
  expect(screen.getByLabelText("API Key")).toHaveAttribute("type", "text");
  fireEvent.click(screen.getByRole("button", { name: "隐藏 API Key" }));
  expect(screen.getByLabelText("API Key")).toHaveAttribute("type", "password");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyOperation: "keep" }));
  expect(props.onDiscover).not.toHaveBeenCalled();
});

it("edits directly, refreshes once, and saves the matching discovery", async () => {
  const { props } = setup();
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "replacement" } });
  fireEvent.click(screen.getByRole("button", { name: "刷新协议" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "刷新协议" })).toBeEnabled());
  expect(props.onDiscover).toHaveBeenCalledWith({ baseUrl: discovery.source_url, apiKey: "replacement" }, expect.any(AbortSignal));
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onDiscover).toHaveBeenCalledTimes(1);
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyOperation: "replace", discovery }));
});

it("refreshes stored credentials without revealing secrets and preserves results on failure", async () => {
  const { props } = setup({ onDiscover: vi.fn().mockRejectedValue(new Error("fixture failure")) });
  fireEvent.click(screen.getByRole("button", { name: "刷新协议" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("fixture failure");
  expect(props.onDiscover).toHaveBeenCalledWith({ credentialId: "account" }, expect.any(AbortSignal));
  expect(screen.getByText("Chat Completions")).toBeInTheDocument();
  expect(props.onRevealKey).not.toHaveBeenCalled();
});

it("requires secret access and discards reveals after close", async () => {
  const denied = setup({ hasSecretAccess: false });
  fireEvent.click(screen.getByRole("button", { name: "显示 API Key" }));
  expect(denied.props.onRequestSecretAccess).toHaveBeenCalled();
  expect(denied.props.onRevealKey).not.toHaveBeenCalled();
  denied.view.unmount();
  let resolve!: (key: string) => void;
  const { view, props } = setup({ onRevealKey: vi.fn(() => new Promise<string>((done) => { resolve = done; })) });
  fireEvent.click(screen.getByRole("button", { name: "显示 API Key" }));
  view.rerender(<UiLocaleProvider><CredentialDialog {...props} open={false} /></UiLocaleProvider>);
  await act(async () => resolve("late-secret"));
  view.rerender(<UiLocaleProvider><CredentialDialog {...props} /></UiLocaleProvider>);
  expect(screen.getByLabelText("API Key")).toHaveValue("");
});
