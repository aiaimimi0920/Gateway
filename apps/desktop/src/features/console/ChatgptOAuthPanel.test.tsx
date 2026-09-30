import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import { ChatgptOAuthPanel } from "./ChatgptOAuthPanel";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: translate }) }));
const translate = (zh: string) => zh;
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/chatgpt-auth", () => ({ createChatgptAuthApi: () => api }));
const client: GatewayApiClient = { request: async () => { throw new Error("Unexpected transport"); } };
const session = (status: string) => ({ session: { id: "login", status, message: status,
  authorizationUrl: "https://auth.openai.com/oauth/authorize", credentialId: null } });
const props = { client, managementToken: "operator", secretGrant: "grant", providerId: "chatgpt",
  disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };

beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(session("waiting_user")); api.act.mockResolvedValue(session("cancelled")); });
afterEach(() => { vi.useRealTimers(); });

it("requires secret confirmation and never creates a session without it", async () => {
  render(<ChatgptOAuthPanel {...props} secretGrant={null} />);
  fireEvent.click(screen.getByRole("button", { name: "通过 ChatGPT 登录" }));
  expect(props.onRequestSecretAccess).toHaveBeenCalledOnce();
  expect(api.create).not.toHaveBeenCalled();
});

it("polls authorization and saves only after explicit confirmation", async () => {
  api.get.mockResolvedValue(session("ready"));
  const view = render(<ChatgptOAuthPanel {...props} />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 ChatGPT 登录" })); });
  expect(api.create).toHaveBeenCalledWith("chatgpt", "free", "grant");
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(api.act).not.toHaveBeenCalled();
  api.act.mockResolvedValue(session("succeeded"));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "保存到凭证池" })); });
  expect(api.act).toHaveBeenCalledWith("login", "import", "grant", undefined);
  expect(props.onSaved).toHaveBeenCalledOnce();
  view.unmount();
  expect(api.act).toHaveBeenCalledTimes(1);
});

it("cancels an in-flight creation after the dialog unmounts", async () => {
  let resolve!: (value: ReturnType<typeof session>) => void;
  api.create.mockReturnValue(new Promise((done) => { resolve = done; }));
  const view = render(<ChatgptOAuthPanel {...props} />);
  fireEvent.click(screen.getByRole("button", { name: "通过 ChatGPT 登录" }));
  view.unmount();
  await act(async () => { resolve(session("waiting_user")); });
  expect(api.act).toHaveBeenCalledWith("login", "cancel");
  expect(props.onSaved).not.toHaveBeenCalled();
});

const expired = () => new GatewayApiError("Session unavailable", 400, "chatgpt_auth_session_unavailable");

it("keeps checking ready sessions and allows a new login after expiry", async () => {
  api.get.mockResolvedValueOnce(session("ready")).mockRejectedValueOnce(expired());
  render(<ChatgptOAuthPanel {...props} />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 ChatGPT 登录" })); });
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(screen.getByRole("button", { name: "保存到凭证池" })).toBeEnabled();
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(screen.queryByRole("button", { name: "保存到凭证池" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "通过 ChatGPT 登录" })).toBeEnabled();
  expect(screen.getByRole("alert")).toHaveTextContent("登录会话已过期或不可用，请重新登录。");
  const calls = api.get.mock.calls.length;
  await act(async () => { await vi.advanceTimersByTimeAsync(6000); });
  expect(api.get).toHaveBeenCalledTimes(calls);
});

it.each(["保存到凭证池", "取消登录"])("recovers when %s races session expiry", async (button) => {
  api.get.mockResolvedValue(session("ready"));
  api.act.mockRejectedValue(expired());
  render(<ChatgptOAuthPanel {...props} />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 ChatGPT 登录" })); });
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: button })); });
  expect(screen.getByRole("button", { name: "通过 ChatGPT 登录" })).toBeEnabled();
  expect(props.onSaved).not.toHaveBeenCalled();
});

it("retains authorized material after a transient import failure", async () => {
  api.get.mockResolvedValue(session("ready"));
  api.act.mockRejectedValueOnce(new GatewayApiError("Model discovery unavailable", 502));
  render(<ChatgptOAuthPanel {...props} />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 ChatGPT 登录" })); });
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "保存到凭证池" })); });
  expect(screen.getByRole("button", { name: "保存到凭证池" })).toBeEnabled();
  expect(screen.getByRole("button", { name: "通过 ChatGPT 登录" })).toBeDisabled();
  api.act.mockResolvedValueOnce(session("succeeded"));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "保存到凭证池" })); });
  expect(props.onSaved).toHaveBeenCalledOnce();
});

it("refreshes saved credentials when polling recovers a lost import response", async () => {
  api.get.mockResolvedValueOnce(session("ready")).mockResolvedValue(session("succeeded"));
  api.act.mockRejectedValueOnce(new GatewayApiError("Connection interrupted", 502));
  const view = render(<ChatgptOAuthPanel {...props} />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 ChatGPT 登录" })); });
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "保存到凭证池" })); });
  expect(props.onSaved).not.toHaveBeenCalled();
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(props.onSaved).toHaveBeenCalledOnce();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  await act(async () => { await vi.advanceTimersByTimeAsync(6000); });
  expect(props.onSaved).toHaveBeenCalledOnce();
  expect(api.act).toHaveBeenCalledTimes(1);
  view.unmount();
  expect(api.act).toHaveBeenCalledTimes(1);
});
