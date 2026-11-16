import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AccessKeyCopyButton } from "./AccessKeyCopyButton";

const t = (zh: string) => zh;

afterEach(() => vi.useRealTimers());

it("shows a check and Copy only after success, then restores the copy icon", async () => {
  vi.useFakeTimers();
  let resolve!: (ok: boolean) => void;
  const onCopy = vi.fn(() => new Promise<boolean>((done) => { resolve = done; }));
  render(<AccessKeyCopyButton disabled={false} onCopy={onCopy} t={t} />);
  const button = screen.getByRole("button", { name: "复制 Key" });
  expect(button).toHaveTextContent("复制");
  expect(button).toHaveAttribute("data-copy-state", "idle");
  fireEvent.click(button);
  expect(button).toBeDisabled();
  expect(button).toHaveAttribute("aria-busy", "true");
  expect(button).toHaveAccessibleName("复制 Key");
  await act(async () => resolve(true));
  expect(button).toBeEnabled();
  expect(button).toHaveAttribute("title", "已复制");
  expect(button).toHaveAttribute("data-copy-state", "copied");
  expect(screen.getByRole("status")).toHaveTextContent("已复制");
  await act(async () => { vi.advanceTimersByTime(2400); });
  expect(button).toHaveAttribute("data-copy-state", "idle");
  expect(button).not.toHaveTextContent("已复制");
  expect(onCopy).toHaveBeenCalledTimes(1);
});

it("does not show success on failure and releases the pending state", async () => {
  render(<AccessKeyCopyButton disabled={false} onCopy={async () => false} t={t} />);
  const button = screen.getByRole("button", { name: "复制 Key" });
  await act(async () => { fireEvent.click(button); });
  expect(button).toBeEnabled();
  expect(button).toHaveTextContent("复制失败");
  expect(button).toHaveAttribute("data-copy-state", "error");
  expect(screen.getByRole("status")).toHaveTextContent("复制失败");
});
