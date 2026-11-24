import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import { LumaFilesPanel } from "./LumaFilesPanel";
import type { LumaFilesProps } from "./useLumaFiles";
import { LUMA_FILE_LIMIT } from "../../api/console/luma-files";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const id = "d290f1ee-6c54-4b01-90e6-d701748f0851";
const binding = { revision: "r1", providerId: "agents", credentialId: "saved" };
const file = { id, filename: "参考.png", mime_type: "image/png", size_bytes: 3, purpose: "reference", state: "pending", created_at: "2026-11-20T00:00:00Z" };
const list = { ...binding, result: { action: "list", data: [file], has_more: false, next_cursor: null } };
function fixture() {
  const request = vi.fn(async (_path: string, schema, _options?: GatewayApiRequestOptions) => schema.parse(list));
  const props: LumaFilesProps = { client: { request } as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
    providerId: "agents", credentialId: "saved", expectedRevision: "r1", disabled: false, onRequestSecretAccess: vi.fn() };
  return { request, props };
}
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
function reply(f: ReturnType<typeof fixture>, result: unknown) {
  f.request.mockImplementationOnce(async (_path, schema) => schema.parse({ ...binding, result }));
}
function chooseFile(file = new File([new Uint8Array([1, 2, 3])], "参考.png", { type: "image/png" })) {
  fireEvent.change(screen.getByLabelText("上传文件（最大 5 GiB）"), { target: { files: [file] } });
}

it("uploads bytes then refreshes pending to ready before copying, and confirms deletion", async () => {
  const f = fixture(); const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  render(<LumaFilesPanel {...f.props} />); expect(f.request).not.toHaveBeenCalled();
  chooseFile(); fireEvent.change(screen.getByLabelText("文件用途"), { target: { value: "reference" } });
  reply(f, { action: "upload", file }); await click("上传文件");
  await waitFor(() => expect(screen.getByLabelText("file_id")).toHaveValue(id));
  expect(f.request).toHaveBeenLastCalledWith("/v1/internal/gateway/console/luma-agents-files", expect.anything(), expect.objectContaining({
    method: "POST", cache: "no-store", managementToken: "owner", secretGrant: "grant", notifyAuthenticationFailure: false,
    body: { providerId: "agents", credentialId: "saved", expectedRevision: "r1", operation: { action: "upload", filename: "参考.png", purpose: "reference", data: "AQID" } },
  }));
  expect(screen.getByRole("button", { name: "复制 file_id" })).toBeDisabled();
  reply(f, { action: "get", file: { ...file, state: "ready" } }); await click("刷新文件状态"); await click("复制 file_id");
  expect(writeText).toHaveBeenCalledWith(id);
  await click("删除文件"); expect(f.request).toHaveBeenCalledTimes(2);
  expect(screen.getByText(/无法通过 API 恢复/)).toBeInTheDocument(); await click("取消删除"); expect(f.request).toHaveBeenCalledTimes(2);
  await click("删除文件"); reply(f, { action: "delete", id }); await click("确认删除文件");
  expect(f.request.mock.calls[2][2]!.body).toEqual(expect.objectContaining({ operation: { action: "delete", id } }));
  expect(screen.getByText("没有文件")).toBeInTheDocument(); expect(screen.queryByLabelText("file_id")).not.toBeInTheDocument();
});

it("reads each page explicitly and propagates the opaque cursor", async () => {
  const f = fixture(); reply(f, { ...list.result, has_more: true, next_cursor: "opaque+/=" });
  render(<LumaFilesPanel {...f.props} />); await click("读取文件列表"); expect(f.request).toHaveBeenCalledOnce();
  reply(f, { action: "list", data: [], has_more: false, next_cursor: null }); await click("下一页");
  expect(f.request.mock.calls[1][2]!.body).toEqual(expect.objectContaining({ operation: { action: "list", cursor: "opaque+/=" } }));
  expect(screen.getByText("没有文件")).toBeInTheDocument(); expect(screen.queryByRole("button", { name: "下一页" })).not.toBeInTheDocument();
});

it.each(["expectedRevision", "managementToken", "credentialId", "providerId", "secretGrant"] as const)("aborts and discards late files when %s changes", async (key) => {
  const f = fixture(); let resolve!: (v: unknown) => void;
  f.request.mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
  const view = render(<LumaFilesPanel {...f.props} />); await click("读取文件列表");
  const signal = f.request.mock.calls[0][2]!.signal!;
  view.rerender(<LumaFilesPanel {...f.props} {...{ [key]: "changed" }} />);
  expect(signal.aborted).toBe(true); await act(async () => resolve(list));
  expect(screen.queryByLabelText("file_id")).not.toBeInTheDocument();
});

it("revokes a deletion confirmation and selected upload when scope becomes dirty", async () => {
  const f = fixture(); const view = render(<LumaFilesPanel {...f.props} />);
  chooseFile(); await click("读取文件列表"); await click("删除文件");
  view.rerender(<LumaFilesPanel {...f.props} disabled />);
  expect(screen.queryByRole("button", { name: "确认删除文件" })).not.toBeInTheDocument();
  view.rerender(<LumaFilesPanel {...f.props} />);
  expect(screen.getByRole("button", { name: "上传文件" })).toBeDisabled(); expect(f.request).toHaveBeenCalledOnce();
});

it("requires consent without auto-uploading when the grant arrives", async () => {
  const f = fixture(); const view = render(<LumaFilesPanel {...f.props} secretGrant={null} />);
  chooseFile(); await click("上传文件"); expect(f.props.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(f.request).not.toHaveBeenCalled();
  view.rerender(<LumaFilesPanel {...f.props} />); expect(f.request).not.toHaveBeenCalled(); expect(screen.getByRole("button", { name: "上传文件" })).toBeDisabled();
});

it("rejects empty and over-limit files before reading or networking", async () => {
  const f = fixture(); render(<LumaFilesPanel {...f.props} />);
  for (const size of [0, LUMA_FILE_LIMIT + 1]) {
    const file = new File([], "a.png"); Object.defineProperty(file, "size", { value: size }); chooseFile(file); await click("上传文件");
    expect(screen.getByText(/请选择不超过 5 GiB/)).toBeInTheDocument();
  }
  expect(f.request).not.toHaveBeenCalled();
});

it("never replays uncertain writes, does not expose errors, and invalidates stale ready metadata", async () => {
  const f = fixture(); reply(f, { ...list.result, data: [{ ...file, state: "ready" }] });
  render(<LumaFilesPanel {...f.props} />); await click("读取文件列表");
  f.request.mockRejectedValueOnce(new Error("private signed-url")); await click("刷新文件状态");
  expect(screen.queryByLabelText("file_id")).not.toBeInTheDocument(); expect(screen.queryByText(/private signed-url/)).not.toBeInTheDocument();
  await click("读取文件列表"); await click("删除文件"); f.request.mockRejectedValueOnce(new Error("uncertain")); await click("确认删除文件");
  expect(f.request).toHaveBeenCalledTimes(4); expect(screen.queryByRole("button", { name: "确认删除文件" })).not.toBeInTheDocument();
  expect(screen.getByText(/可能已生效/)).toBeInTheDocument();
});

it("rejects foreign account or file metadata and stops pending I/O on unmount", async () => {
  const f = fixture(); f.request.mockResolvedValueOnce({ ...list, credentialId: "foreign" });
  const view = render(<LumaFilesPanel {...f.props} />); await click("读取文件列表"); expect(screen.queryByLabelText("file_id")).not.toBeInTheDocument();
  await click("读取文件列表"); reply(f, { action: "get", file: { ...file, id: "d290f1ee-6c54-4b01-90e6-d701748f0852" } });
  await click("刷新文件状态"); expect(screen.queryByLabelText("file_id")).not.toBeInTheDocument();
  f.request.mockImplementation(() => new Promise(() => {})); await click("读取文件列表"); await click("停止等待");
  expect(f.request.mock.calls[3][2]!.signal!.aborted).toBe(true); expect(f.request).toHaveBeenCalledTimes(4);
  await click("读取文件列表"); view.unmount(); expect(f.request.mock.calls[4][2]!.signal!.aborted).toBe(true);
});
