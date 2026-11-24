import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import { LumaFilesPanel } from "./LumaFilesPanel";
import type { LumaFilesProps } from "./useLumaFiles";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const id = "d290f1ee-6c54-4b01-90e6-d701748f0851";
function fixture() {
  const request = vi.fn(async (_path: string, schema, _options?: GatewayApiRequestOptions) => schema.parse({
    revision: "r1", providerId: "agents", credentialId: "saved", result: { action: "upload", file: {
      id, filename: "参考.png", mime_type: "image/png", size_bytes: 3, purpose: "input", state: "pending",
      created_at: "2026-11-20T00:00:00Z", expires_at: "2099-01-01T00:00:00Z", user_id: "owner-tag",
    } },
  }));
  const props: LumaFilesProps = { client: { request } as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
    providerId: "agents", credentialId: "saved", expectedRevision: "r1", disabled: false, onRequestSecretAccess: vi.fn() };
  return { request, props };
}
const choose = (file: File) => fireEvent.change(screen.getByLabelText("上传文件（最大 5 GiB）"), { target: { files: [file] } });
const upload = async () => act(async () => { fireEvent.click(screen.getByRole("button", { name: "上传文件" })); });

it("sends a large file as binary with the exact saved-account binding", async () => {
  const f = fixture(); render(<LumaFilesPanel {...f.props} />);
  const file = new File([new Uint8Array(8 * 1024 * 1024 + 1)], "参考.png", { type: "image/png" });
  choose(file); await upload();
  const body = f.request.mock.calls[0][2]!.body as FormData;
  expect(body).toBeInstanceOf(FormData); expect(body.get("file")).toBeInstanceOf(File);
  expect(JSON.parse(body.get("metadata") as string)).toEqual({ providerId: "agents", credentialId: "saved", expectedRevision: "r1",
    upload: { filename: "参考.png", mime_type: "image/png", size_bytes: file.size, purpose: "input" } });
  expect(f.request).toHaveBeenCalledOnce(); expect(screen.getByRole("button", { name: "复制 file_id" })).toBeDisabled();
});

it("sends optional expiration and attribution through presign even for a small file", async () => {
  const f = fixture(); render(<LumaFilesPanel {...f.props} />);
  choose(new File(["abc"], "参考.png", { type: "image/png" }));
  fireEvent.change(screen.getByLabelText("过期时间（可选）"), { target: { value: "2099-01-01T08:00" } });
  fireEvent.change(screen.getByLabelText("用户标记（可选）"), { target: { value: "owner-tag" } });
  await upload();
  const body = f.request.mock.calls[0][2]!.body as FormData;
  expect(JSON.parse(body.get("metadata") as string).upload).toMatchObject({ user_id: "owner-tag", expires_at: new Date("2099-01-01T08:00").toISOString() });
  expect(screen.getByText(/owner-tag/)).toBeInTheDocument();
});

it("rejects past TTL, control characters and unknown large-file MIME before networking", async () => {
  const f = fixture(); render(<LumaFilesPanel {...f.props} />);
  choose(new File(["abc"], "a.png", { type: "image/png" }));
  fireEvent.change(screen.getByLabelText("过期时间（可选）"), { target: { value: "2020-01-01T08:00" } });
  await upload(); expect(f.request).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("过期时间（可选）"), { target: { value: "" } });
  fireEvent.change(screen.getByLabelText("用户标记（可选）"), { target: { value: "a\x01b" } });
  await upload(); expect(f.request).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("用户标记（可选）"), { target: { value: "" } });
  const file = new File([], "a.unknown"); Object.defineProperty(file, "size", { value: 9 * 1024 * 1024 }); choose(file);
  await upload(); expect(f.request).not.toHaveBeenCalled();
});

it("cancels binary uploads on account changes and discards their late results", async () => {
  const f = fixture(); let done!: (value: unknown) => void;
  f.request.mockImplementationOnce(() => new Promise((resolve) => { done = resolve; }));
  const view = render(<LumaFilesPanel {...f.props} />);
  choose(new File(["abc"], "a.png", { type: "image/png" }));
  fireEvent.change(screen.getByLabelText("用户标记（可选）"), { target: { value: "tag" } });
  await upload(); view.rerender(<LumaFilesPanel {...f.props} credentialId="other" />);
  expect(f.request.mock.calls[0][2]!.signal!.aborted).toBe(true);
  await act(async () => done({})); expect(screen.queryByLabelText("file_id")).not.toBeInTheDocument();
  expect(screen.getByLabelText("用户标记（可选）")).toHaveValue("");
});
