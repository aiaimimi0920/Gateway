import { NeuroProvider } from "@neuro/ui";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import type { CursorInput, CursorSession } from "../../api/console/cursor-auth";
import { CursorImportPanel } from "./CursorImportPanel";
import type { CursorImportProps } from "./useCursorImport";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const authorizationUrl = `https://www.cursor.com/loginDeepControl?challenge=${"x".repeat(43)}&uuid=aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee&mode=login&redirectTarget=cli`;
const receipt = (input: CursorInput): CursorSession => ({
  id: input.attemptId, providerId: input.providerId, expectedRevision: input.expectedRevision,
  targetCredentialId: `cursor-${input.attemptId}`, status: "waiting_user", authorizationUrl,
  authReceived: false, accountMatched: false, credentialPersisted: false,
  expiresAt: null, savedRevision: null, errorCode: null, inferenceStatus: "not_checked",
});
function setup() {
  // Exercise the real panel, hook and Cursor API adapter; only the HTTP boundary is synthetic.
  const request = vi.fn(async (_path: string, _schema: unknown, options?: GatewayApiRequestOptions) => ({
    session: receipt(options?.body as CursorInput),
  }));
  const props: CursorImportProps = {
    client: { request } as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
    providerId: "cursor", expectedRevision: "r1", operations: new Map(), disabled: false,
    onSaved: vi.fn(), onRequestSecretAccess: vi.fn(),
  };
  render(<CursorImportPanel {...props} />, { wrapper: NeuroProvider });
  return { request, props };
}
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

it("starts browser import without an email and shows the accepted link after one click", async () => {
  const { request, props } = setup();
  const start = screen.getByRole("button", { name: "生成 Cursor 授权链接" });
  expect(start).toBeEnabled();
  expect(screen.queryByLabelText("本人 Cursor 邮箱")).not.toBeInTheDocument();
  expect(screen.getByText(/无需填写邮箱/)).toBeInTheDocument();
  expect(screen.getByText(/此按钮不会自动弹出浏览器/)).toBeInTheDocument();
  await act(async () => { fireEvent.click(start); });
  expect(request).toHaveBeenCalledExactlyOnceWith(
    "/v1/internal/gateway/console/cursor-auth-sessions", expect.anything(),
    expect.objectContaining({ method: "POST", secretGrant: "grant",
      body: expect.objectContaining({ expectedRevision: "r1" }) }),
  );
  const body = request.mock.calls[0][2]?.body as CursorInput;
  expect(body).not.toHaveProperty("expectedEmail");
  expect(body).not.toHaveProperty("accessToken");
  expect(screen.getByRole("button", { name: "等待网页确认" })).toBeDisabled();
  expect(screen.getByText(/授权链接已生成。请点击下方/)).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "打开授权页面" })).toHaveAttribute("href", authorizationUrl);
  fireEvent.click(screen.getByRole("button", { name: "等待网页确认" }));
  expect(request).toHaveBeenCalledOnce();
  expect(props.operations.size).toBe(1);
  expect(props.onSaved).not.toHaveBeenCalled();
});

it("keeps a rejected creation visible instead of presenting a browser-waiting state", async () => {
  const { request, props } = setup();
  request.mockRejectedValueOnce(new GatewayApiError("private details", 400, "cursor_import_target_invalid"));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "生成 Cursor 授权链接" })); });
  expect(screen.getByText("未接纳导入：cursor_import_target_invalid")).toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "打开授权页面" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "生成 Cursor 授权链接" })).toBeEnabled();
  expect(props.operations.size).toBe(0);
  expect(props.onSaved).not.toHaveBeenCalled();
});
