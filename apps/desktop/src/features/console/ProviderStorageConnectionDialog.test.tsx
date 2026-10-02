import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import type { StorageConnection, StorageSecretChanges } from "./providerStorageConnection";
import { ProviderStorageConnectionDialog } from "./ProviderStorageConnectionDialog";

const base = { providerLabel: "NVIDIA", target: "storage" as const, defaultPath: "/tmp/refill", configuredSecrets: [], disabled: false, onCancel: vi.fn(), t: (zh: string) => zh };
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
