import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { GoogleWebJsonField } from "./GoogleWebJsonField";

const source = JSON.stringify({ schemaVersion: 1, provider: "aistudio-web", authUser: "0", cookies: [] });
it("imports pasted JSON only on explicit submit and clears transient material", () => {
  const imported = vi.fn();
  render(<GoogleWebJsonField provider="aistudio-web" disabled={false} onImport={imported} />);
  fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { value: source } });
  expect(imported).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "导入 JSON" }));
  expect(imported).toHaveBeenCalledWith(source);
  expect(screen.getByLabelText("网页会话 JSON")).toHaveValue("");
  expect(localStorage.getItem("google-web-json")).toBeNull();
});

it.each(["not-json-private-secret", JSON.stringify({ provider: "gemini-web" }), "x".repeat(65537)])("rejects malformed, foreign and oversized material without echoing input", (input) => {
  const imported = vi.fn();
  render(<GoogleWebJsonField provider="aistudio-web" disabled={false} onImport={imported} />);
  fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { value: input } });
  fireEvent.click(screen.getByRole("button", { name: "导入 JSON" }));
  expect(imported).not.toHaveBeenCalled();
  expect(screen.getByRole("alert").textContent).not.toContain("private-secret");
});

it("reads a selected JSON file without importing until confirmed", async () => {
  const imported = vi.fn();
  render(<GoogleWebJsonField provider="aistudio-web" disabled={false} onImport={imported} />);
  fireEvent.change(screen.getByLabelText("选择 JSON 文件"), { target: { files: [{ size: source.length, text: async () => source }] } });
  await waitFor(() => expect(screen.getByLabelText("网页会话 JSON")).toHaveValue(source));
  expect(imported).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "导入 JSON" }));
  expect(imported).toHaveBeenCalledWith(source);
});

it.each(["unmount", "disable"])("discards late file reads after %s", async (change) => {
  let resolve!: (value: string) => void;
  const imported = vi.fn();
  const view = render(<GoogleWebJsonField provider="aistudio-web" disabled={false} onImport={imported} />);
  fireEvent.change(screen.getByLabelText("选择 JSON 文件"), { target: { files: [{ size: 100, text: () => new Promise<string>(yes => { resolve = yes; }) }] } });
  if (change === "unmount") view.unmount();
  else view.rerender(<GoogleWebJsonField provider="aistudio-web" disabled onImport={imported} />);
  await act(async () => resolve(source));
  expect(imported).not.toHaveBeenCalled();
  if (change === "disable") expect(screen.getByLabelText("网页会话 JSON")).toHaveValue("");
});

it("does not display callback exceptions that might contain sensitive material", () => {
  render(<GoogleWebJsonField provider="aistudio-web" disabled={false} onImport={() => { throw new Error("private-secret"); }} />);
  fireEvent.change(screen.getByLabelText("网页会话 JSON"), { target: { value: source } });
  fireEvent.click(screen.getByRole("button", { name: "导入 JSON" }));
  expect(screen.getByRole("alert")).not.toHaveTextContent("private-secret");
  expect(screen.getByLabelText("网页会话 JSON")).toHaveValue("");
});
