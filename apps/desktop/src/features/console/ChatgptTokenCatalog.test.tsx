import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";

const catalog = { clientVersion: "0.155.1", checkedAt: "now", models: [
  { slug: "live-model", visibility: "list", supported_in_api: true },
  { slug: "hidden", visibility: "hide", supported_in_api: true },
] };
function setup(discover = vi.fn().mockResolvedValue(catalog)) {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const props = { open: true, mode: "add" as const, providerOptions: [{ id: "chatgpt", label: "ChatGPT" }],
    existingCredentialIds: [], locked: false, hasSecretAccess: true, onOpenChange: vi.fn(), onRequestSecretAccess: vi.fn(),
    onSubmit: vi.fn(), supportsBasispoints: () => true, renderAuthentication: () => <div>OAuth</div>, onDiscoverChatgptModels: discover };
  const view = render(<UiLocaleProvider><CredentialDialog {...props} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "token" } });
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "fixture" } });
  fireEvent.change(screen.getByLabelText("ChatGPT Account ID"), { target: { value: "fixture-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-token" } });
  return { props, view };
}

it("reads the real catalog before token import and does not add hidden models", async () => {
  const { props } = setup();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(props.onSubmit).toHaveBeenCalled());
  expect(props.onDiscoverChatgptModels).toHaveBeenCalledWith({ accessToken: "fixture-token", accountId: "fixture-account" }, expect.any(AbortSignal));
  expect(props.onSubmit.mock.calls[0][0]).toMatchObject({ supportedModelsText: "live-model", apiKeyValue: "fixture-token" });
});

it("leaves a failed catalog import open without saving", async () => {
  const { props } = setup(vi.fn().mockRejectedValue(new Error("catalog offline")));
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("catalog offline");
  expect(props.onSubmit).not.toHaveBeenCalled();
  expect(props.onOpenChange).not.toHaveBeenCalledWith(false);
});

it("refreshes an existing account by ID inside the editor and stages models until Save", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const discover = vi.fn().mockResolvedValue(catalog); const submit = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="edit" providerOptions={[{ id: "chatgpt", label: "ChatGPT" }]}
    existingCredentialIds={["fixture"]} locked={false} hasSecretAccess supportsBasispoints={() => true}
    initialValue={{ providerId: "chatgpt", credentialId: "fixture", accountName: "", baseUrl: "", enabled: true,
      supportedModelsText: "declared", apiKeyOperation: "keep", apiKeyValue: "" }}
    onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={submit} onDiscoverChatgptModels={discover} />
  </UiLocaleProvider>);
  fireEvent.click(screen.getByRole("button", { name: "拉取模型" }));
  await waitFor(() => expect(screen.getByLabelText("拉取模型列表")).toHaveValue("live-model"));
  expect(screen.getByLabelText("手动模型列表")).toHaveValue("declared");
  expect(discover).toHaveBeenCalledWith({ credentialId: "fixture" }, expect.any(AbortSignal));
  expect(submit).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ supportedModelsText: "declared\nlive-model", apiKeyOperation: "keep" }));
});

it("discards a late token catalog after close or secret-grant loss", async () => {
  for (const revoked of [false, true]) {
    let resolve!: (value: typeof catalog) => void;
    const { props, view } = setup(vi.fn(() => new Promise((done) => { resolve = done; })));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    view.rerender(<UiLocaleProvider><CredentialDialog {...props} open={revoked} hasSecretAccess={!revoked} /></UiLocaleProvider>);
    await act(async () => resolve(catalog));
    expect(props.onSubmit).not.toHaveBeenCalled();
    view.unmount();
  }
});

it("does not repopulate fetched models after the user deliberately clears the token catalog", async () => {
  const { props } = setup();
  fireEvent.click(screen.getByRole("button", { name: "拉取模型" }));
  await waitFor(() => expect(screen.getByLabelText("拉取模型列表")).toHaveValue("live-model"));
  fireEvent.change(screen.getByLabelText("拉取模型列表"), { target: { value: "" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ fetchedModelsText: "", supportedModelsText: "" }));
  expect(props.onDiscoverChatgptModels).toHaveBeenCalledTimes(1);
});
