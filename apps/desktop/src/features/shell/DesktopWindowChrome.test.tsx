import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DesktopWindowChrome } from "./DesktopWindowChrome";

afterEach(() => {
  cleanup();
  delete window.__GATEWAY_WINDOW_CHROME__;
});

describe("desktop window chrome", () => {
  it("leaves ordinary browser pages without native controls or shortcut interception", () => {
    render(<DesktopWindowChrome />);
    expect(screen.queryByRole("group", { name: "窗口操作" })).toBeNull();
    expect(fireEvent.keyDown(document, { key: "g", ctrlKey: true, shiftKey: true })).toBe(true);
  });

  it("keeps connection and window actions available before authentication", async () => {
    const send = vi.fn();
    window.__GATEWAY_WINDOW_CHROME__ = { send };
    render(<DesktopWindowChrome />);
    expect(send).toHaveBeenCalledWith("ready");
    for (const [label, action] of [
      ["连接设置", "connection"], ["最小化", "minimize"],
      ["最大化或还原", "toggle-maximize"], ["关闭", "close"],
    ]) {
      await userEvent.click(screen.getByRole("button", { name: label }));
      expect(send).toHaveBeenLastCalledWith(action);
    }
  });

  it("preserves native shortcuts and removes listeners when unmounted", () => {
    const send = vi.fn();
    window.__GATEWAY_WINDOW_CHROME__ = { send };
    const { unmount } = render(<DesktopWindowChrome />);
    fireEvent.keyDown(document, { key: "G", ctrlKey: true, shiftKey: true });
    expect(send).toHaveBeenLastCalledWith("connection");
    fireEvent.keyDown(document, { key: "q", ctrlKey: true });
    expect(send).toHaveBeenLastCalledWith("close");
    unmount();
    send.mockClear();
    expect(fireEvent.keyDown(document, { key: "g", ctrlKey: true, shiftKey: true })).toBe(true);
    expect(send).not.toHaveBeenCalled();
  });

  it("supports drag and double-click without stealing toolbar-button interaction", () => {
    const send = vi.fn();
    window.__GATEWAY_WINDOW_CHROME__ = { send };
    render(<><DesktopWindowChrome /><div data-tauri-drag-region data-testid="drag">
      <button type="button">Back</button>
    </div></>);
    send.mockClear();
    fireEvent.mouseDown(screen.getByRole("button", { name: "Back" }), { button: 0 });
    expect(send).not.toHaveBeenCalled();
    fireEvent.mouseDown(screen.getByTestId("drag"), { button: 0, detail: 1 });
    expect(send).toHaveBeenLastCalledWith("drag");
    fireEvent.mouseDown(screen.getByTestId("drag"), { button: 0, detail: 2 });
    expect(send).toHaveBeenLastCalledWith("toggle-maximize");
  });
});
