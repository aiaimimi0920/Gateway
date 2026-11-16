import { render as renderUi, screen, waitFor } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { type ReactNode, useState } from "react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import type { StorageConnection, StorageSecretChanges } from "./providerStorageConnection";
import { ProviderStorageConnectionDialog } from "./ProviderStorageConnectionDialog";

const base = { providerLabel: "NVIDIA", target: "storage" as const, defaultPath: "/tmp/refill", configuredSecrets: [], disabled: false, onCancel: vi.fn(), t: (zh: string) => zh };
const render = (ui: ReactNode) => renderUi(ui, { wrapper: ({ children }) => <NeuroProvider>{children}</NeuroProvider> });
it("collects explicit S3 configuration and sends secrets separately", async () => {
  const user = userEvent.setup();
  const onSave = vi.fn<(connection: StorageConnection, secrets: StorageSecretChanges) => boolean>(() => true);
  render(<ProviderStorageConnectionDialog {...base} onSave={onSave} />);
  await user.selectOptions(screen.getByLabelText("存储协议"), "s3");
  await user.type(screen.getByLabelText("服务地址"), "https://storage.example.invalid");
  await user.type(screen.getByLabelText("Bucket"), "fixture");
  await user.type(screen.getByLabelText("Access Key ID"), "synthetic-id");
  await user.type(screen.getByLabelText("Secret Access Key"), "synthetic-secret");
  await user.click(screen.getByRole("button", { name: "保存连接" }));
  expect(onSave).toHaveBeenCalledExactlyOnceWith(
    { type: "s3", endpoint: "https://storage.example.invalid", bucket: "fixture", region: "auto", prefix: "" },
    { access_key_id: { operation: "replace", value: "synthetic-id" }, secret_access_key: { operation: "replace", value: "synthetic-secret" } },
  );
  expect(JSON.stringify(onSave.mock.calls[0][0])).not.toContain("synthetic");
});
it("does not reuse saved auth after changing the destination", async () => {
  const user = userEvent.setup();
  const onSave = vi.fn<(connection: StorageConnection, secrets: StorageSecretChanges) => boolean>(() => true);
  render(<ProviderStorageConnectionDialog {...base} configuredSecrets={["password"]}
    connection={{ type: "webdav", endpoint: "https://first.example.invalid", directory: "pool", username: "fixture" }} onSave={onSave} />);
  await user.clear(screen.getByLabelText("服务地址"));
  await user.type(screen.getByLabelText("服务地址"), "https://second.example.invalid");
  await user.click(screen.getByRole("button", { name: "保存连接" }));
  expect(onSave).not.toHaveBeenCalled();
  expect(screen.getByRole("alert")).toHaveTextContent("不能自动沿用");
});
it("requires an explicit warning checkbox for an HTTP connection", async () => {
  const user = userEvent.setup();
  const onSave = vi.fn<(connection: StorageConnection, secrets: StorageSecretChanges) => boolean>(() => true);
  render(<ProviderStorageConnectionDialog {...base} onSave={onSave} />);
  await user.selectOptions(screen.getByLabelText("存储协议"), "webdav");
  await user.type(screen.getByLabelText("服务地址"), "http://localhost:9999/dav");
  await user.click(screen.getByRole("button", { name: "保存连接" }));
  expect(onSave).not.toHaveBeenCalled();
  await user.click(screen.getByRole("checkbox", { name: /我理解 HTTP/ }));
  await user.click(screen.getByRole("button", { name: "保存连接" }));
  expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ allow_insecure_http: true }), {});
});
it("preserves an unfinished secret through temporary save busy and cancels without saving", async () => {
  const user = userEvent.setup();
  const onSave = vi.fn<(connection: StorageConnection, secrets: StorageSecretChanges) => boolean>(() => true);
  const onCancel = vi.fn();
  const connection = { type: "webdav" as const, endpoint: "https://example.invalid", directory: "pool" };
  const view = render(<ProviderStorageConnectionDialog {...base} connection={connection} onSave={onSave} onCancel={onCancel} />);
  await user.type(screen.getByLabelText("WebDAV 密码"), "synthetic-secret");
  view.rerender(<ProviderStorageConnectionDialog {...base} disabled connection={connection} onSave={onSave} onCancel={onCancel} />);
  expect(screen.getByLabelText("WebDAV 密码")).toHaveValue("synthetic-secret");
  view.rerender(<ProviderStorageConnectionDialog {...base} connection={connection} onSave={onSave} onCancel={onCancel} />);
  await user.keyboard("{Escape}");
  expect(onSave).not.toHaveBeenCalled();
  expect(onCancel).toHaveBeenCalled();
  view.unmount();
  expect(screen.queryByDisplayValue("synthetic-secret")).not.toBeInTheDocument();
});

it("keeps configured authentication blank and sends only an explicit clear after an identity change", async () => {
  const user = userEvent.setup();
  const onSave = vi.fn(() => false);
  const onCancel = vi.fn();
  render(<ProviderStorageConnectionDialog {...base} configuredSecrets={["password"]}
    connection={{ type: "webdav", endpoint: "https://first.example.invalid", username: "fixture", directory: "pool" }}
    onSave={onSave} onCancel={onCancel} />);
  const password = screen.getByLabelText("WebDAV 密码");
  expect(password).toHaveValue("");
  expect(password).toHaveAttribute("type", "password");
  expect(password).toHaveAttribute("placeholder", "已配置；留空保留");
  await user.click(screen.getByRole("button", { name: "保存连接" }));
  expect(onSave).toHaveBeenLastCalledWith(expect.objectContaining({ endpoint: "https://first.example.invalid" }), {});
  expect(screen.getByRole("dialog")).toBeInTheDocument();
  expect(onCancel).not.toHaveBeenCalled();
  await user.clear(screen.getByLabelText("服务地址"));
  await user.type(screen.getByLabelText("服务地址"), "https://second.example.invalid");
  await user.click(screen.getByRole("checkbox", { name: "WebDAV 密码：清除已保存的认证值" }));
  await user.click(screen.getByRole("button", { name: "保存连接" }));
  expect(onSave).toHaveBeenLastCalledWith(expect.objectContaining({ endpoint: "https://second.example.invalid" }), { password: { operation: "clear" } });
});

it("drops old protocol fields and unfinished authentication when switching protocols", async () => {
  const user = userEvent.setup();
  const onSave = vi.fn(() => false);
  render(<ProviderStorageConnectionDialog {...base}
    connection={{ type: "webdav", endpoint: "https://first.example.invalid", username: "old-user", directory: "old-folder" }} onSave={onSave} />);
  await user.type(screen.getByLabelText("WebDAV 密码"), "synthetic-old-password");
  await user.selectOptions(screen.getByLabelText("存储协议"), "s3");
  expect(screen.queryByDisplayValue("synthetic-old-password")).not.toBeInTheDocument();
  await user.type(screen.getByLabelText("服务地址"), "https://s3.example.invalid");
  await user.type(screen.getByLabelText("Bucket"), "fixture-bucket");
  await user.click(screen.getByRole("button", { name: "保存连接" }));
  expect(onSave).toHaveBeenCalledExactlyOnceWith({ type: "s3", endpoint: "https://s3.example.invalid", bucket: "fixture-bucket", region: "auto", prefix: "" }, {});
});

it("requires a username for a replacement WebDAV password and associates the failure with the dialog", async () => {
  const user = userEvent.setup();
  const onSave = vi.fn(() => false);
  render(<ProviderStorageConnectionDialog {...base} connection={{ type: "webdav", endpoint: "https://example.invalid" }} onSave={onSave} />);
  await user.type(screen.getByLabelText("WebDAV 密码"), "synthetic-password");
  await user.click(screen.getByRole("button", { name: "保存连接" }));
  expect(onSave).not.toHaveBeenCalled();
  expect(screen.getByRole("dialog")).toHaveAccessibleDescription(expect.stringContaining("密码需要同时填写用户名"));
});

it("returns focus to the same card flip action if saving disables the original opener", async () => {
  function Example() {
    const [open, setOpen] = useState(false);
    const [disabled, setDisabled] = useState(false);
    return <article><div className="nt-provider-card__back">
      <button className="nt-provider-card__flip">翻面</button>
      <button disabled={disabled} onClick={() => setOpen(true)}>连接入口</button>
    </div>{open ? <ProviderStorageConnectionDialog {...base} onCancel={() => setOpen(false)}
      onSave={() => { setDisabled(true); return true; }} /> : null}</article>;
  }
  const user = userEvent.setup();
  render(<Example />);
  await user.click(screen.getByRole("button", { name: "连接入口" }));
  expect(screen.getByLabelText("存储协议")).toHaveFocus();
  await user.click(screen.getByRole("button", { name: "保存连接" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "翻面" })).toHaveFocus());
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "连接入口" })).toBeDisabled();
});
