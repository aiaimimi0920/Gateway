import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import type { LumaAuthSession } from "../../api/console/luma-auth";
import { LumaRefreshPanel } from "./LumaRefreshPanel";
import type { LumaOAuthProps } from "./useLumaOAuth";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
function fixture() {
  let status: LumaAuthSession["status"] = "ready";
  const request = vi.fn(async (path: string, schema, _options?: GatewayApiRequestOptions) => {
    if (path.endsWith("/import") || path.endsWith("/resume")) status = "importing";
    if (path.endsWith("/cancel")) status = "cancelled";
    return schema.parse({ session: { id: "refresh", providerId: "mcp", expectedRevision: "r1", status,
      authorizationUrl: null, redirectUri: "http://127.0.0.1:43567/callback", credentialId: status === "succeeded" ? "stable" : null,
      targetCredentialId: "stable", savedRevision: status === "succeeded" ? "r2" : null, errorCode: null } });
  });
  const props: LumaOAuthProps & { onActivityChange(active: boolean): void } = { client: { request } as GatewayApiClient,
    managementToken: "owner", secretGrant: "grant", providerId: "mcp", credentialId: "stable", expectedRevision: "r1",
    operations: new Map(), disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn(), onActivityChange: vi.fn() };
  return { props, request, setStatus: (next: LumaAuthSession["status"]) => { status = next; } };
}
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
const poll = async () => { await act(async () => { await vi.advanceTimersByTimeAsync(1500); }); };
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

it("requires explicit refresh confirmation and observes the original account result", async () => {
  const f = fixture(); render(<LumaRefreshPanel {...f.props} />);
  await click("刷新账号授权");
  expect(f.request).toHaveBeenCalledTimes(1);
  expect(f.request.mock.calls[0][2]).toMatchObject({ secretGrant: "grant", body: { providerId: "mcp", credentialId: "stable", expectedRevision: "r1" } });
  await click("确认刷新授权");
  expect(screen.getByText("正在刷新并保存")).toBeInTheDocument();
  f.setStatus("succeeded"); await poll();
  expect(f.props.onSaved).toHaveBeenCalledOnce();
  expect(f.request.mock.calls.filter(([p]) => p.endsWith("/import"))).toHaveLength(1);
});
it("retains a lost import across reopen without sending it again", async () => {
  const f = fixture(); const view = render(<LumaRefreshPanel {...f.props} />);
  await click("刷新账号授权"); f.request.mockRejectedValueOnce(new Error("lost")); await click("确认刷新授权");
  view.unmount(); render(<LumaRefreshPanel {...f.props} />);
  expect(screen.queryByRole("button", { name: "确认刷新授权" })).not.toBeInTheDocument();
  f.setStatus("importing"); await poll(); f.setStatus("succeeded"); await poll();
  expect(f.props.onSaved).toHaveBeenCalledOnce();
  expect(f.request.mock.calls.filter(([p]) => p.endsWith("/import"))).toHaveLength(1);
});
it("resumes saving rather than importing again after a terminal unconfirmed result", async () => {
  const f = fixture(); const view = render(<LumaRefreshPanel {...f.props} />);
  await click("刷新账号授权"); await click("确认刷新授权");
  f.setStatus("save_unconfirmed"); await poll();
  view.rerender(<LumaRefreshPanel {...f.props} disabled expectedRevision="r2" />);
  await click("恢复保存");
  expect(f.request.mock.calls.at(-1)?.[0]).toMatch(/\/resume$/);
  expect(f.request.mock.calls.at(-1)?.[2]?.body).toEqual({ expectedRevision: "r1" });
  f.setStatus("succeeded"); await poll(); expect(f.props.onSaved).toHaveBeenCalledOnce();
});
it("asks for secret access before preparing and never performs implicit refresh", async () => {
  const f = fixture(); render(<LumaRefreshPanel {...f.props} secretGrant={null} />);
  await click("刷新账号授权"); expect(f.props.onRequestSecretAccess).toHaveBeenCalledOnce();
  expect(f.request).not.toHaveBeenCalled();
});

it("reports a rejected secret-access prompt without sending a request", async () => {
  const f = fixture(); f.props.onRequestSecretAccess = vi.fn().mockRejectedValue(new Error("prompt closed"));
  render(<LumaRefreshPanel {...f.props} secretGrant={null} />);
  await click("刷新账号授权"); expect(f.request).not.toHaveBeenCalled();
  expect(screen.getByText(/操作未完成/)).toBeInTheDocument();
});

it("allows cancelling preparation and explicitly preparing again", async () => {
  const f = fixture(); render(<LumaRefreshPanel {...f.props} />);
  await click("刷新账号授权"); await click("取消刷新");
  expect(screen.getByRole("button", { name: "刷新账号授权" })).toBeEnabled();
  expect(f.request.mock.calls.some(([p]) => p.endsWith("/import"))).toBe(false);
});

it("uses the current revision for a new explicit refresh after success", async () => {
  const f = fixture(); const view = render(<LumaRefreshPanel {...f.props} />);
  await click("刷新账号授权"); await click("确认刷新授权");
  f.setStatus("succeeded"); await poll();
  view.rerender(<LumaRefreshPanel {...f.props} expectedRevision="r2" />);
  f.request.mockImplementationOnce(async (_path, schema) => schema.parse({ session: {
    id: "next-refresh", providerId: "mcp", expectedRevision: "r2", status: "ready", authorizationUrl: null,
    redirectUri: "http://127.0.0.1:43567/callback", credentialId: null, targetCredentialId: "stable", savedRevision: null, errorCode: null,
  } }));
  await click("刷新账号授权");
  expect(f.request.mock.calls.at(-1)?.[2]?.body).toMatchObject({ expectedRevision: "r2" });
  expect(screen.getByRole("button", { name: "确认刷新授权" })).toBeEnabled();
});
