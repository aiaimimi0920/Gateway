import { act, fireEvent, render as renderBase, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { expect, it, vi } from "vitest";
import { freebuffCatalogResponse } from "./freebuffCatalog";
import { FreebuffCatalogPanel, type FreebuffCatalogProps } from "./FreebuffCatalogPanel";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const render = (ui: Parameters<typeof renderBase>[0]) => renderBase(ui, { wrapper: NeuroProvider });
function owner() {
  const request = vi.fn().mockResolvedValue({ revision: "r1", providerId: "pool", credentialId: "b",
    businessOrigin: "https://codebuff.com", client: "cli", models: [{ key: "stable", displayName: "模型", tagline: "测试",
      access: "open", dataUse: "service", premium: false, multimodal: true, warning: null, badges: [] }] });
  const props: FreebuffCatalogProps = { client: { request }, providerId: "pool", credentialId: "b", expectedRevision: "r1",
    managementToken: "owner", secretGrant: "grant", disabled: false, onRequestSecretAccess: vi.fn() };
  return { props, request };
}
function choose() {
  fireEvent.change(screen.getByLabelText("业务站点"), { target: { value: "https://codebuff.com" } });
  fireEvent.change(screen.getByLabelText("客户端"), { target: { value: "cli" } });
}
it("does not request on mount and requires explicit business target/client before an exact read", async () => {
  const { props, request } = owner(); render(<FreebuffCatalogPanel {...props} />);
  expect(request).not.toHaveBeenCalled(); expect(screen.getByRole("button", { name: "读取目录" })).toBeDisabled(); choose();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取目录" })); });
  expect(request).toHaveBeenCalledExactlyOnceWith("/v1/internal/gateway/console/freebuff-model-catalog", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner", secretGrant: "grant", signal: expect.any(AbortSignal),
    body: { providerId: "pool", credentialId: "b", expectedRevision: "r1", businessOrigin: "https://codebuff.com", client: "cli" },
  }));
  expect(screen.getByText("模型")).toBeInTheDocument();
});
it.each([
  { credentialId: "other" }, { providerId: "other" }, { expectedRevision: "r2" }, { managementToken: "other" },
  { secretGrant: "other" }, { secretGrant: null }, { disabled: true }, { client: { request: vi.fn() } },
])("aborts and rejects a late response after context change: %j", async (patch) => {
  const { props, request } = owner(); let resolve!: (value: unknown) => void;
  const value = await request(); request.mockClear(); request.mockImplementation(() => new Promise((done) => { resolve = done; }));
  const view = render(<FreebuffCatalogPanel {...props} />); choose();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取目录" })); });
  const signal = request.mock.calls[0][2].signal; view.rerender(<FreebuffCatalogPanel {...props} {...patch} />);
  expect(signal.aborted).toBe(true); await act(async () => { resolve(value); });
  expect(screen.queryByText("模型")).not.toBeInTheDocument();
});
it("aborts on close/unmount and explicit cancellation; ignores an uncancellable old transport", async () => {
  const { props, request } = owner(); let resolve!: (value: unknown) => void; const value = await request(); request.mockClear();
  request.mockImplementation(() => new Promise((done) => { resolve = done; }));
  const view = render(<FreebuffCatalogPanel {...props} />); choose();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取目录" })); });
  const first = request.mock.calls[0][2].signal;
  fireEvent.click(screen.getByRole("button", { name: "取消读取" })); expect(first.aborted).toBe(true);
  await act(async () => { resolve(value); }); expect(screen.queryByText("模型")).not.toBeInTheDocument();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取目录" })); });
  view.unmount(); expect(request.mock.calls[1][2].signal.aborted).toBe(true);
});
it.each([{ disabled: true }, { expectedRevision: "" }, { managementToken: null }])("blocks without a clean saved context: %j", (patch) => {
  const { props, request } = owner(); render(<FreebuffCatalogPanel {...props} {...patch} />); choose();
  expect(screen.getByRole("button", { name: "读取目录" })).toBeDisabled(); expect(request).not.toHaveBeenCalled();
});
it("requests grant restoration without fetching or automatically retrying", () => {
  const { props, request } = owner(); const view = render(<FreebuffCatalogPanel {...props} secretGrant={null} />); choose();
  fireEvent.click(screen.getByRole("button", { name: "读取目录" })); expect(props.onRequestSecretAccess).toHaveBeenCalledOnce();
  view.rerender(<FreebuffCatalogPanel {...props} />); expect(request).not.toHaveBeenCalled();
});
it.each(["revision", "providerId", "credentialId", "businessOrigin", "client"])("rejects foreign response binding: %s", async (field) => {
  const { props, request } = owner(); const value = await request(); request.mockClear(); request.mockResolvedValue({ ...value, [field]: "foreign" });
  render(<FreebuffCatalogPanel {...props} />); choose();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取目录" })); });
  expect(screen.getByText(/目录响应与当前账号/)).toBeInTheDocument(); expect(screen.queryByText("模型")).not.toBeInTheDocument();
});
it.each(["handle", "fetchId", "apiKey"])("never accepts wire capability or secrets in the display DTO: %s", async (field) => {
  const { request } = owner(); const value = await request(); value.models[0][field] = "sensitive";
  expect(freebuffCatalogResponse.safeParse(value).success).toBe(false);
});
it("clears results on target/client changes and shows failure without leaking upstream diagnostics", async () => {
  const { props, request } = owner(); render(<FreebuffCatalogPanel {...props} />); choose();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取目录" })); }); expect(screen.getByText("模型")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("客户端"), { target: { value: "desktop" } }); expect(screen.queryByText("模型")).not.toBeInTheDocument();
  request.mockRejectedValue(new Error("upstream synthetic-secret"));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取目录" })); });
  expect(screen.getByText(/目录读取失败/)).toBeInTheDocument(); expect(screen.queryByText(/synthetic-secret/)).not.toBeInTheDocument();
  expect(request).toHaveBeenCalledTimes(2);
});
it("restores an explicitly saved business target without reading on mount or borrowing another target", () => {
  const { props, request } = owner(); const view = render(<FreebuffCatalogPanel {...props} savedBusiness={{ origin: "https://codebuff.com", client: "cli" }} />);
  expect(screen.getByLabelText("业务站点")).toHaveValue("https://codebuff.com");
  expect(screen.getByLabelText("客户端")).toHaveValue("cli"); expect(request).not.toHaveBeenCalled();
  view.rerender(<FreebuffCatalogPanel {...props} savedBusiness={{ origin: "https://www.codebuff.com", client: "desktop" }} />);
  expect(screen.getByLabelText("业务站点")).toHaveValue("https://www.codebuff.com");
  expect(screen.getByLabelText("客户端")).toHaveValue("desktop"); expect(request).not.toHaveBeenCalled();
  view.rerender(<FreebuffCatalogPanel {...props} />);
  expect(screen.getByLabelText("业务站点")).toHaveValue(""); expect(screen.getByLabelText("客户端")).toHaveValue("");
});
it("requires an explicit selection for each catalog snapshot and applies only the validated display response", async () => {
  const { props, request } = owner(); const apply = vi.fn().mockReturnValue(false);
  render(<FreebuffCatalogPanel {...props} onApply={apply} />); choose();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取目录" })); });
  expect(screen.getByRole("button", { name: "应用到账号" })).toBeDisabled();
  fireEvent.click(screen.getByRole("checkbox", { name: "选择 stable" }));
  fireEvent.click(screen.getByRole("button", { name: "应用到账号" }));
  expect(screen.getByRole("alertdialog")).toHaveTextContent("本操作不购买会话，实际调用仍需逐次批准");
  expect(screen.queryByText(/公共调用尚未接入/)).not.toBeInTheDocument();
  expect(apply).not.toHaveBeenCalled(); fireEvent.click(screen.getByRole("button", { name: "确认应用" }));
  expect(apply).toHaveBeenCalledWith(await request.mock.results[0].value, ["stable"]);
  expect(screen.getByText("模型")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取目录" })); });
  expect(screen.getByRole("checkbox", { name: "选择 stable" })).not.toBeChecked();
  expect(screen.getByRole("button", { name: "应用到账号" })).toBeDisabled();
  fireEvent.click(screen.getByRole("checkbox", { name: "选择 stable" })); apply.mockReturnValue(true);
  fireEvent.click(screen.getByRole("button", { name: "应用到账号" }));
  fireEvent.click(screen.getByRole("button", { name: "确认应用" })); expect(screen.queryByText("模型")).not.toBeInTheDocument();
  expect(request).toHaveBeenCalledTimes(2);
});
