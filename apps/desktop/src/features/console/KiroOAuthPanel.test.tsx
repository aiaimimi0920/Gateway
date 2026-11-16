import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import type { KiroAuthSession } from "../../api/console/kiro-auth";
import { KiroOAuthPanel } from "./KiroOAuthPanel";
import type { KiroOAuthProps } from "./useKiroOAuth";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/kiro-auth", async (original) => ({ ...await original<typeof import("../../api/console/kiro-auth")>(), createKiroAuthApi: () => api }));
const client: GatewayApiClient = { request: async () => { throw new Error("Unexpected transport"); } };
const session = (status: KiroAuthSession["status"]): KiroAuthSession => ({ id: "login", providerId: "kiro", expectedRevision: "r1", status,
  verificationUri: "https://device.sso.us-east-1.amazonaws.com/", verificationUriComplete: null, userCode: "ABCD-EFGH", pollIntervalSecs: 5, expiresInSecs: 590,
  credentialId: status === "succeeded" ? "account" : null, targetCredentialId: null, savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
const props: KiroOAuthProps = { client, managementToken: "operator", secretGrant: "grant", providerId: "kiro", expectedRevision: "r1",
  operations: new Map(), disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
const check = async () => { await act(async () => { await vi.advanceTimersByTimeAsync(1500); }); };
function fill(method = "builder_id") {
  fireEvent.change(screen.getByLabelText("授权目录"), { target: { value: method } });
  fireEvent.click(screen.getByLabelText("codewhisperer:conversations"));
  fireEvent.change(screen.getByLabelText("Machine ID"), { target: { value: "A".repeat(64) } });
}
beforeEach(() => {
  props.operations.clear(); vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(session("waiting_user"));
  api.get.mockResolvedValue(session("waiting_user")); api.act.mockResolvedValue(session("cancelled"));
});
afterEach(() => vi.useRealTimers());

it("does not invent a directory, minimum scope or machine and does not POST on timed GET checks", async () => {
  render(<KiroOAuthPanel {...props} />);
  expect(screen.getByLabelText("授权目录")).toHaveValue(""); expect(screen.getByLabelText("Machine ID")).toHaveValue("");
  for (const checkbox of screen.getAllByRole("checkbox")) expect(checkbox).not.toBeChecked();
  expect(screen.getByRole("button", { name: "通过 Kiro 登录" })).toBeDisabled();
  fill(); await click("通过 Kiro 登录"); await check(); await check();
  expect(api.get).toHaveBeenCalledTimes(2); expect(api.act).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce(session("polling")); await click("检查授权");
  expect(api.act).toHaveBeenCalledExactlyOnceWith("login", "poll", expect.objectContaining({ expectedRevision: "r1" }), "grant", undefined);
  expect(screen.getByRole("button", { name: "检查授权" })).toBeDisabled();
});

it("keeps Identity Center directory separate from the provider inference Region", async () => {
  render(<KiroOAuthPanel {...props} />); fill("identity_center");
  expect(screen.getByRole("button", { name: "通过 Kiro 登录" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Directory Region"), { target: { value: "eu-west-1" } });
  fireEvent.change(screen.getByLabelText("Start URL"), { target: { value: "https://organization.awsapps.com/start" } });
  await click("通过 Kiro 登录");
  expect(api.create).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ directory: { authMethod: "identity_center", directoryRegion: "eu-west-1", startUrl: "https://organization.awsapps.com/start" }, scopes: ["codewhisperer:conversations"], machineId: "A".repeat(64) }), "grant");
});

it.each(["", "device", "a".repeat(63), "a".repeat(65), "设备"])("rejects an invalid new machine: %j", (machineId) => {
  render(<KiroOAuthPanel {...props} />); fill(); fireEvent.change(screen.getByLabelText("Machine ID"), { target: { value: machineId } });
  expect(screen.getByRole("button", { name: "通过 Kiro 登录" })).toBeDisabled(); expect(api.create).not.toHaveBeenCalled();
});

it("only resumes the captured poll after a renewed grant and never rebases it", async () => {
  const view = render(<KiroOAuthPanel {...props} />); fill(); await click("通过 Kiro 登录");
  view.rerender(<KiroOAuthPanel {...props} secretGrant={null} />); await click("检查授权");
  expect(props.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(api.act).not.toHaveBeenCalled();
  view.rerender(<KiroOAuthPanel {...props} secretGrant="renewed" expectedRevision="r2" />); await act(async () => {});
  expect(api.act).not.toHaveBeenCalled(); expect(screen.getByRole("button", { name: "检查授权" })).toBeDisabled();
});

it("never saves automatically, and restores an ambiguous accepted save after close/reopen", async () => {
  const view = render(<KiroOAuthPanel {...props} />); fill(); await click("通过 Kiro 登录");
  api.get.mockResolvedValue(session("ready")); await check(); expect(api.act).not.toHaveBeenCalled();
  api.act.mockRejectedValueOnce(new Error("lost")); await click("保存到凭证池"); view.unmount();
  render(<KiroOAuthPanel {...props} />);
  expect(screen.getByRole("button", { name: "通过 Kiro 登录" })).toBeDisabled();
  expect(screen.queryByRole("button", { name: "保存到凭证池" })).not.toBeInTheDocument();
  api.get.mockResolvedValue(session("succeeded")); await check();
  expect(props.onSaved).toHaveBeenCalledOnce(); expect(api.create).toHaveBeenCalledOnce(); expect(api.act).toHaveBeenCalledTimes(1);
});

it.each(["kiro_login_save_rejected", "unknown"])("only an explicit pre-admission rejection unlocks save retry: %s", async (code) => {
  render(<KiroOAuthPanel {...props} />); fill(); await click("通过 Kiro 登录");
  api.get.mockResolvedValue(session("ready")); await check();
  api.act.mockRejectedValueOnce(new GatewayApiError("Rejected", 400, code)); await click("保存到凭证池");
  expect(Boolean(screen.queryByRole("button", { name: "保存到凭证池" }))).toBe(code === "kiro_login_save_rejected");
  await check(); expect(api.act).toHaveBeenCalledTimes(1); expect(props.onSaved).not.toHaveBeenCalled();
});

it("keeps save_unconfirmed locked and closing does not cancel or resubmit it", async () => {
  const activity = vi.fn(); const view = render(<KiroOAuthPanel {...props} onActivityChange={activity} />); fill(); await click("通过 Kiro 登录");
  api.get.mockResolvedValue(session("ready")); await check();
  api.act.mockResolvedValueOnce(session("save_unconfirmed")); await click("保存到凭证池");
  expect(activity).toHaveBeenLastCalledWith(true); expect(screen.getByRole("button", { name: "通过 Kiro 登录" })).toBeDisabled();
  view.unmount(); expect(api.act).toHaveBeenCalledTimes(1); expect(props.onSaved).not.toHaveBeenCalled();
});
