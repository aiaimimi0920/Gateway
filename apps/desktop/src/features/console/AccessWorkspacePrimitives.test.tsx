import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { CopyButton } from "./AccessWorkspacePrimitives";

afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); });

it.each([
  { language: "Chinese", t: (zh: string) => zh, label: undefined, caption: "复制", copied: "已复制" },
  { language: "English", t: (_zh: string, en: string) => en, label: undefined, caption: "Copy", copied: "Copied" },
  { language: "custom", t: (zh: string) => zh, label: "复制明文", caption: "复制明文", copied: "已复制" },
])("keeps the $language visible copy caption unchanged on success and restores its icon", async ({ t, label, caption, copied }) => {
  vi.useFakeTimers();
  userEvent.setup();
  const write = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
  render(<CopyButton t={t} value="https://gateway.test/v1" label={label} variant="outline" />);
  const button = screen.getByRole("button", { name: caption });
  const visibleCaption = () => button.querySelector("span:not(.nt-visually-hidden)")?.textContent;
  expect(visibleCaption()).toBe(caption);
  expect(button.querySelector(".lucide-copy")).not.toBeNull();
  await act(async () => { fireEvent.click(button); });
  expect(write).toHaveBeenCalledWith("https://gateway.test/v1");
  expect(visibleCaption()).toBe(caption);
  expect(button.querySelector(".lucide-check")).not.toBeNull();
  expect(button).toHaveClass("nt-console-copy--ok");
  expect(button).toHaveAttribute("title", copied);
  expect(button.querySelector('[aria-live="polite"]')).toHaveTextContent(copied);
  await act(async () => { vi.advanceTimersByTime(2400); });
  expect(visibleCaption()).toBe(caption);
  expect(button.querySelector(".lucide-copy")).not.toBeNull();
  expect(button.querySelector(".lucide-check")).toBeNull();
  expect(button.querySelector('[aria-live="polite"]')).toHaveTextContent("");
});

it("preserves clipboard failure feedback without showing a success check", async () => {
  userEvent.setup();
  vi.spyOn(navigator.clipboard, "writeText").mockRejectedValue(new Error("clipboard denied"));
  render(<CopyButton t={(zh) => zh} value="https://gateway.test/v1" />);
  const button = screen.getByRole("button", { name: "复制" });
  await act(async () => { fireEvent.click(button); });
  expect(button).toHaveTextContent("复制失败");
  expect(button).toHaveClass("nt-console-copy--fail");
  expect(button.querySelector(".lucide-check")).toBeNull();
});
