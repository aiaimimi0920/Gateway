import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { CursorImportPanel } from "./CursorImportPanel";
import type { CursorImportProps } from "./useCursorImport";
import type { GatewayApiClient } from "../../api/client";
import type { CursorSession } from "../../api/console/cursor-auth";
import { cursorAuthorizationUrl } from "./cursorAuthorizationUrl";
const flow = vi.hoisted(() => ({ start: vi.fn(), busy: false, pending: false, paused: false, session: undefined as CursorSession | undefined }));
vi.mock("./useCursorImport", () => ({ useCursorImport: () => flow }));
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const props = (): CursorImportProps => ({ client: {} as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
  providerId: "cursor", expectedRevision: "r1", operations: new Map(), disabled: false, onSaved: vi.fn(), onRequestSecretAccess: vi.fn() });
beforeEach(() => { vi.clearAllMocks(); flow.session = undefined; });
it("clears the manual secret immediately after explicit submit and distinguishes missing material", () => {
  const input = props();
  render(<CursorImportPanel {...input} savedAccount={{ name: "owner@example.test", hasToken: false, expiresAt: "2099-01-01T00:00:00Z" }} />, { wrapper: NeuroProvider });
  expect(screen.getByText(/缺少已保存的访问令牌/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox"));
  expect(screen.queryByLabelText("本人 Cursor 邮箱")).not.toBeInTheDocument();
  expect(screen.queryByText(/请在 Cursor 网页选择/)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "校验并导入" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Cursor access token"), { target: { value: "synthetic-token" } });
  fireEvent.click(screen.getByRole("button", { name: "校验并导入" }));
  expect(flow.start).toHaveBeenCalledWith("synthetic-token");
  expect(screen.getByLabelText("Cursor access token")).toHaveValue("");
});
it("shows expired custody without pretending the account is logged out or inference is usable", () => {
  render(<CursorImportPanel {...props()} savedAccount={{ name: "owner@example.test", hasToken: true, expiresAt: "2000-01-01T00:00:00Z" }} />, { wrapper: NeuroProvider });
  expect(screen.getByText(/令牌已过期/)).toHaveTextContent("需要重新授权");
  expect(screen.getByText(/不是 Cloud Agents API Key/)).toHaveTextContent("导入不会启动本地 Cursor、发送推理请求或启用付费回落");
  expect(screen.getByText(/不同账号不会覆盖已保存凭据/)).toBeInTheDocument();
});
it("confirms credential persistence without inferring routing or a successful model call", () => {
  flow.session = { id: "attempt", providerId: "cursor", expectedRevision: "r1", targetCredentialId: "saved-account",
    status: "succeeded", authorizationUrl: null, authReceived: true, accountMatched: true, credentialPersisted: true,
    expiresAt: null, savedRevision: "r2", errorCode: null, inferenceStatus: "not_checked" };
  render(<CursorImportPanel {...props()} credentialId="saved-account" />, { wrapper: NeuroProvider });
  expect(screen.getByText(/凭据已保存并读回确认/)).toHaveTextContent("本次导入未发送推理请求；账号是否启用，请查看账号列表中的调度开关");
  expect(screen.getByText(/材料已收到：是/)).toHaveTextContent("令牌身份已核验：是 · 已保存：是");
  expect(screen.queryByText(/调用仍禁用|账号已启用|推理已验证/)).not.toBeInTheDocument();
});
it("starts reauthorization without using the display name as an email constraint", () => {
  render(<CursorImportPanel {...props()} credentialId="saved-account" savedAccount={{ name: "我的账号", hasToken: true, expiresAt: null }} />, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: "生成 Cursor 授权链接" }));
  expect(flow.start).toHaveBeenCalledExactlyOnceWith(undefined);
});
it("does not describe a verified different identity as a successful account replacement", () => {
  flow.session = { id: "attempt", providerId: "cursor", expectedRevision: "r1", targetCredentialId: "saved-account",
    status: "failed", authorizationUrl: null, authReceived: true, accountMatched: true, credentialPersisted: false,
    expiresAt: null, savedRevision: null, errorCode: "cursor_import_identity_conflict", inferenceStatus: "not_checked" };
  render(<CursorImportPanel {...props()} />, { wrapper: NeuroProvider });
  expect(screen.getByText(/原有凭据未被覆盖/)).toBeInTheDocument();
  expect(screen.getByText(/材料已收到：是/)).toHaveTextContent("令牌身份已核验：是 · 已保存：未确认");
  expect(screen.queryByText(/身份匹配：是/)).not.toBeInTheDocument();
});
it("permits only a fixed Cursor PKCE authorization link with exact public parameters", () => {
  const url = `https://www.cursor.com/loginDeepControl?challenge=${"x".repeat(43)}&uuid=aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee&mode=login&redirectTarget=cli`;
  expect(cursorAuthorizationUrl(url)).toBe(url);
  for (const invalid of [url.replace("redirectTarget=cli", "redirectTarget=https://attacker.invalid"), `${url}&accessToken=secret`,
    `${url}&uuid=duplicate`, `${url}#fragment`, url.replace("www.cursor.com", "attacker.invalid"), url.replace("mode=login", "mode=other")]) {
    expect(cursorAuthorizationUrl(invalid)).toBeNull();
  }
});
