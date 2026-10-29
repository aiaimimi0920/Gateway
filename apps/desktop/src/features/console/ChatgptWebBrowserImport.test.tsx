import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ChatgptWebBrowserImport, type WebBrowserCapture } from "./ChatgptWebBrowserImport";

const material = { authToken: "web-token", cookieHeader: "session=fixture", userAgent: "Browser", deviceId: "device", expiresAt: "2099-01-01T00:00:00Z" };
it("starts owned browser login without a port or page and returns material to the import owner", async () => {
  const capture = vi.fn().mockResolvedValueOnce({ material, accountName: "fixture@example.test" });
  const onMaterial = vi.fn();
  render(<ChatgptWebBrowserImport providerId="web" capture={capture} disabled={false} onMaterial={onMaterial} onClear={vi.fn()} />);
  expect(screen.queryByLabelText("浏览器调试端口")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
  await screen.findByText("已读取账号：fixture@example.test");
  expect(capture).toHaveBeenLastCalledWith({ providerId: "web", operation: "login", requestId: expect.any(String) }, expect.any(AbortSignal));
  expect(onMaterial).toHaveBeenCalledWith(expect.objectContaining({ accessToken: "web-token" }), "fixture@example.test");
});
it("aborts pending capture when permission/provider context changes", async () => {
  let finish!: (result: Awaited<ReturnType<WebBrowserCapture>>) => void;
  const capture = vi.fn<WebBrowserCapture>((input) => input.operation === "cancel" ? Promise.resolve({}) : new Promise((resolve) => { finish = resolve; }));
  const onMaterial = vi.fn();
  const view = render(<ChatgptWebBrowserImport providerId="web" capture={capture} disabled={false} onMaterial={onMaterial} onClear={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
  const signal = capture.mock.calls[0][1];
  view.rerender(<ChatgptWebBrowserImport providerId="web" capture={capture} disabled onMaterial={onMaterial} onClear={vi.fn()} />);
  await act(async () => { finish({ material, accountName: "stale" }); });
  expect(signal.aborted).toBe(true);
  expect(screen.queryByLabelText("ChatGPT 页面")).not.toBeInTheDocument();
  expect(onMaterial).not.toHaveBeenCalled();
  expect(capture).toHaveBeenLastCalledWith({ providerId: "web", operation: "cancel", requestId: capture.mock.calls[0][0].requestId }, expect.any(AbortSignal));
});

it("explicit cancellation aborts the request and asks the server to close only its login window", async () => {
  const capture = vi.fn<WebBrowserCapture>((input) => input.operation === "cancel" ? Promise.resolve({}) : new Promise(() => {}));
  render(<ChatgptWebBrowserImport providerId="web" capture={capture} disabled={false} onMaterial={vi.fn()} onClear={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
  fireEvent.click(screen.getByRole("button", { name: "取消登录" }));
  expect(capture.mock.calls[0][1].aborted).toBe(true);
  expect(capture.mock.calls[1][0].operation).toBe("cancel");
  expect(screen.getByRole("button", { name: "网页登录" })).toBeEnabled();
});

it("shows an explicit verification action, retains the login transaction, and only imports its final material", async () => {
  vi.useFakeTimers();
  try {
    let finish!: (result: Awaited<ReturnType<WebBrowserCapture>>) => void;
    const capture = vi.fn<WebBrowserCapture>((input) => {
      if (input.operation === "login") return new Promise((resolve) => { finish = resolve; });
      if (input.operation === "status") return Promise.resolve({ phase: "verification_required" });
      if (input.operation === "verify") return Promise.resolve({ phase: "verification_window" });
      return Promise.resolve({});
    });
    const onMaterial = vi.fn();
    const view = render(<ChatgptWebBrowserImport providerId="web" capture={capture} disabled={false} onMaterial={onMaterial} onClear={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
    await act(() => vi.advanceTimersByTimeAsync(750));
    expect(screen.getByText("会话接口要求官网验证，尚未读取账号。")).toBeInTheDocument();
    expect(onMaterial).not.toHaveBeenCalled();
    expect(capture.mock.calls.some(([input]) => input.operation === "verify")).toBe(false);
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "打开验证窗口" })); });
    expect(capture.mock.calls.find(([input]) => input.operation === "verify")?.[0].requestId).toBe(capture.mock.calls[0][0].requestId);
    expect(screen.getByText("请在专用窗口完成官网验证后关闭窗口；请勿复制页面内容。")).toBeInTheDocument();
    await act(async () => { finish({ material, accountName: "fixture@example.test" }); });
    expect(screen.getByText("已读取账号：fixture@example.test")).toBeInTheDocument();
    expect(onMaterial).toHaveBeenCalledTimes(1);
    const calls = capture.mock.calls.length;
    await act(() => vi.advanceTimersByTimeAsync(1500));
    expect(capture).toHaveBeenCalledTimes(calls);
    view.unmount();
  } finally { vi.useRealTimers(); }
});

it("a delayed verification acknowledgement cannot overwrite the later session-read phase", async () => {
  vi.useFakeTimers();
  try {
    let finishLogin!: (result: Awaited<ReturnType<WebBrowserCapture>>) => void;
    let finishVerify!: (result: Awaited<ReturnType<WebBrowserCapture>>) => void;
    let polls = 0;
    const capture = vi.fn<WebBrowserCapture>((input) => {
      if (input.operation === "login") return new Promise((resolve) => { finishLogin = resolve; });
      if (input.operation === "verify") return new Promise((resolve) => { finishVerify = resolve; });
      return Promise.resolve({ phase: ++polls === 1 ? "verification_required" : "session_read" });
    });
    const view = render(<ChatgptWebBrowserImport providerId="web" capture={capture} disabled={false} onMaterial={vi.fn()} onClear={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "网页登录" }));
    await act(() => vi.advanceTimersByTimeAsync(750));
    fireEvent.click(screen.getByRole("button", { name: "打开验证窗口" }));
    await act(() => vi.advanceTimersByTimeAsync(750));
    expect(screen.getByText("正在读取已登录账号…")).toBeInTheDocument();
    await act(async () => { finishVerify({ phase: "verification_window" }); });
    expect(screen.getByText("正在读取已登录账号…")).toBeInTheDocument();
    await act(async () => { finishLogin({ material }); });
    view.unmount();
  } finally { vi.useRealTimers(); }
});
