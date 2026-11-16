import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import { LumaOAuthPanel } from "./LumaOAuthPanel";
import type { LumaOAuthProps } from "./useLumaOAuth";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const id = "b31c16c6-56b9-402b-aeb0-518038289b93";
const initial = { id, providerId: "mcp", expectedRevision: "r1", redirectUri: "http://127.0.0.1:54421/callback",
  status: "prepared", clientId: null as string | null, offlineAccess: false };
function fixture() {
  let view = { ...initial };
  const request = vi.fn(async (path: string, schema, options?: GatewayApiRequestOptions) => {
    if (path.endsWith("luma-client-registrations")) view = { ...view, offlineAccess: (options?.body as { offlineAccess?: boolean })?.offlineAccess ?? false };
    if (path.endsWith("/start")) view = { ...view, status: "registering" };
    return schema.parse({ registration: view });
  });
  const props: LumaOAuthProps = { client: { request } as GatewayApiClient, providerId: "mcp", expectedRevision: "r1",
    managementToken: "owner", secretGrant: "grant", operations: new Map(), disabled: false,
    onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
  return { props, request, registered: () => { view = { ...view, status: "registered", clientId: "registered-client" }; } };
}
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };

it("separates refresh-capable registration from a prior code-only receipt", async () => {
  const f = fixture(); render(<LumaOAuthPanel {...f.props} />);
  await click("准备独立客户端注册");
  fireEvent.click(screen.getByRole("checkbox", { name: "允许刷新账号授权" }));
  await click("准备独立客户端注册");
  expect(f.request.mock.calls.at(-1)?.[2]?.body).toEqual({ providerId: "mcp", expectedRevision: "r1", offlineAccess: true });
  expect(screen.getByText(/将注册支持刷新授权的独立客户端/)).toBeInTheDocument();
  expect(f.request.mock.calls.some(([path]) => path.endsWith("/start"))).toBe(false);
});

it("requires separate prepare and confirmation, then explicit use before login", async () => {
  const f = fixture(); render(<LumaOAuthPanel {...f.props} />);
  expect(f.request).not.toHaveBeenCalled(); await click("准备独立客户端注册");
  expect(f.request.mock.calls[0][2]?.body).toEqual({ providerId: "mcp", expectedRevision: "r1" });
  expect(screen.getByText(/将向 Luma 注册独立客户端/)).toBeInTheDocument();
  await click("确认注册客户端");
  expect(f.request.mock.calls[1][2]?.body).toEqual({ expectedRevision: "r1", confirmRegistration: true });
  f.registered(); await click("检查注册结果");
  expect(f.request.mock.calls[2][2]?.method).toBe("GET");
  expect(screen.getByLabelText("Client ID")).toHaveValue("");
  await click("使用此客户端授权");
  expect(screen.getByLabelText("Client ID")).toHaveValue("registered-client");
  expect(screen.getByLabelText("已登记的回调地址")).toHaveValue(initial.redirectUri);
  expect(screen.getByRole("button", { name: "通过 Luma 登录" })).toBeEnabled();
  expect(f.request.mock.calls.every(([path]) => path.includes("luma-client-registrations"))).toBe(true);
});
it("retains an ambiguous start across close and reopens with GET-only recovery", async () => {
  const f = fixture(); const view = render(<LumaOAuthPanel {...f.props} />); await click("准备独立客户端注册");
  f.request.mockRejectedValueOnce(new Error("private-upstream")); await click("确认注册客户端"); view.unmount();
  render(<LumaOAuthPanel {...f.props} />);
  expect(screen.queryByRole("button", { name: "确认注册客户端" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "准备独立客户端注册" })).not.toBeInTheDocument();
  expect(screen.queryByText("private-upstream")).not.toBeInTheDocument();
  f.registered(); await click("检查注册结果");
  expect(f.request.mock.calls.filter(([path]) => path.endsWith("/start"))).toHaveLength(1);
  expect(screen.getByLabelText("已登记 Client ID")).toHaveValue("registered-client");
});
it("never replays a start when a reclaimed receipt cannot be found", async () => {
  const f = fixture(); render(<LumaOAuthPanel {...f.props} />); await click("准备独立客户端注册"); await click("确认注册客户端");
  f.request.mockRejectedValueOnce(new Error("missing")); await click("检查注册结果");
  expect(screen.queryByRole("button", { name: "确认注册客户端" })).not.toBeInTheDocument();
  expect(f.request.mock.calls.filter(([path]) => path.endsWith("/start"))).toHaveLength(1);
});
it("does not submit registration after revision changes or consent arrives", async () => {
  const f = fixture(); const view = render(<LumaOAuthPanel {...f.props} secretGrant={null} />);
  await click("准备独立客户端注册"); expect(f.props.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(f.request).not.toHaveBeenCalled();
  view.rerender(<LumaOAuthPanel {...f.props} />); expect(f.request).not.toHaveBeenCalled(); await click("准备独立客户端注册");
  view.rerender(<LumaOAuthPanel {...f.props} expectedRevision="r2" />);
  expect(screen.getByRole("button", { name: "确认注册客户端" })).toBeDisabled();
  f.request.mockResolvedValueOnce({ registration: { ...initial, id: "b31c16c6-56b9-402b-aeb0-518038289b94", expectedRevision: "r2" } });
  await click("重新准备注册");
  expect(screen.getByRole("button", { name: "确认注册客户端" })).toBeEnabled();
  expect(f.request.mock.calls.filter(([path]) => path.endsWith("/start"))).toHaveLength(0);
});
it("rejects a mismatched registration response without exposing its client", async () => {
  const f = fixture(); render(<LumaOAuthPanel {...f.props} />); await click("准备独立客户端注册");
  f.request.mockResolvedValueOnce({ registration: { ...initial, id: "other", status: "registered", clientId: "foreign" } });
  await click("确认注册客户端");
  expect(screen.queryByDisplayValue("foreign")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "确认注册客户端" })).not.toBeInTheDocument();
});
it("has no registration action for existing accounts or locked drafts", () => {
  const f = fixture(); const view = render(<LumaOAuthPanel {...f.props} credentialId="saved" />);
  expect(screen.queryByRole("button", { name: "准备独立客户端注册" })).not.toBeInTheDocument();
  view.rerender(<LumaOAuthPanel {...f.props} disabled />);
  expect(screen.getByRole("button", { name: "准备独立客户端注册" })).toBeDisabled();
});
it("allows preparing again after the server proves an attempt expired before it started", async () => {
  const f = fixture(); render(<LumaOAuthPanel {...f.props} />); await click("准备独立客户端注册");
  f.request.mockResolvedValueOnce({ registration: { ...initial, status: "expired" } });
  await click("确认注册客户端");
  expect(screen.getByRole("button", { name: "重新准备注册" })).toBeEnabled();
});
