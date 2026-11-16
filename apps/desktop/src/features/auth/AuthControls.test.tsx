import { useState, type ReactNode } from "react";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Button } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { UiThemeProvider } from "../../theme/UiThemeProvider";
import { LoginPage } from "./LoginPage";
import { BootstrapPage } from "./BootstrapPage";
import { SecretConfirmDialog } from "./SecretConfirmDialog";
import { SessionErrorPage } from "./SessionErrorPage";

function Providers({ children }: { children: ReactNode }) {
  return <UiThemeProvider><UiLocaleProvider>{children}</UiLocaleProvider></UiThemeProvider>;
}

function SecretHarness({ confirm }: { confirm: (token: string) => Promise<void> }) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return <>
    <Button onClick={() => { setError(null); setOpen(true); }}>访问敏感信息</Button>
    <SecretConfirmDialog open={open} onOpenChange={setOpen} busy={busy} error={error} onConfirm={async token => {
      setBusy(true); setError(null);
      try { await confirm(token); }
      catch (failure) { setError("测试授权被拒绝"); throw failure; }
      finally { setBusy(false); }
    }} />
  </>;
}

describe("standard authentication controls", () => {
  beforeEach(() => window.localStorage.clear());

  it("keeps native password validation and submits the unchanged token", async () => {
    const user = userEvent.setup(), submit = vi.fn(async (_token: string) => undefined);
    render(<Providers><LoginPage busy={false} onSubmit={submit} /></Providers>);
    const token = screen.getByLabelText("管理密钥");
    expect(token).toHaveAttribute("type", "password");
    expect(token).toHaveAttribute("autocomplete", "current-password");
    expect(token).toHaveAttribute("name", "management-token");
    await user.click(screen.getByRole("button", { name: "登录" }));
    expect(submit).not.toHaveBeenCalled();
    await user.type(token, " synthetic-token ");
    await user.keyboard("{Enter}");
    expect(submit).toHaveBeenCalledExactlyOnceWith(" synthetic-token ");
  });

  it("keeps the action label stable while busy and displays the actual session error", async () => {
    const user = userEvent.setup(), submit = vi.fn(async () => undefined);
    render(<Providers><LoginPage busy error="测试登录失败" onSubmit={submit} /></Providers>);
    const button = screen.getByRole("button", { name: "登录" });
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-busy", "true");
    expect(screen.getByRole("alert")).toHaveTextContent("测试登录失败");
    await user.click(button);
    expect(submit).not.toHaveBeenCalled();
  });

  it("validates the confirmation locally and preserves the original normalization boundary", async () => {
    const user = userEvent.setup(), submit = vi.fn(async (_token: string) => undefined);
    render(<Providers><BootstrapPage busy={false} onSubmit={submit} /></Providers>);
    const token = screen.getByLabelText("管理密钥", { exact: true }), confirmation = screen.getByLabelText("确认管理密钥");
    expect(token).toHaveAttribute("autocomplete", "new-password");
    await user.type(token, " synthetic-token "); await user.type(confirmation, "different");
    await user.click(screen.getByRole("button", { name: "创建管理员" }));
    expect(screen.getByRole("alert")).toHaveTextContent("两次输入的管理密钥不一致");
    expect(submit).not.toHaveBeenCalled();
    await user.clear(confirmation); await user.type(confirmation, "synthetic-token");
    await user.click(screen.getByRole("button", { name: "创建管理员" }));
    expect(submit).toHaveBeenCalledExactlyOnceWith(" synthetic-token ");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("blocks duplicate initialization and retry actions during busy state", async () => {
    const user = userEvent.setup(), retry = vi.fn(async () => undefined);
    const view = render(<Providers><BootstrapPage busy onSubmit={retry} /></Providers>);
    expect(screen.getByRole("button", { name: "创建管理员" })).toBeDisabled();
    view.rerender(<Providers><SessionErrorPage busy onRetry={retry} /></Providers>);
    await user.click(screen.getByRole("button", { name: "重新初始化" }));
    expect(retry).not.toHaveBeenCalled();
    view.rerender(<Providers><SessionErrorPage busy={false} onRetry={retry} error="测试会话错误" /></Providers>);
    expect(screen.getByRole("alert")).toHaveTextContent("测试会话错误");
    await user.click(screen.getByRole("button", { name: "重新初始化" }));
    expect(retry).toHaveBeenCalledOnce();
  });

  it("focuses the confirmation field, retains failed drafts and clears them on close", async () => {
    const user = userEvent.setup(), confirm = vi.fn(async () => { throw Error("synthetic rejection"); });
    render(<Providers><SecretHarness confirm={confirm} /></Providers>);
    const trigger = screen.getByRole("button", { name: "访问敏感信息" });
    await user.click(trigger);
    const dialog = screen.getByRole("dialog", { name: "确认敏感信息访问权限" });
    const token = within(dialog).getByLabelText("管理密钥");
    await waitFor(() => expect(token).toHaveFocus());
    await user.type(token, "synthetic-secret"); await user.click(within(dialog).getByRole("button", { name: "确认" }));
    await within(dialog).findByRole("alert");
    expect(token).toHaveValue("synthetic-secret");
    expect(dialog).toBeVisible();
    expect(confirm).toHaveBeenCalledExactlyOnceWith("synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "取消" }));
    await waitFor(() => expect(trigger).toHaveFocus());
    await user.click(trigger);
    expect(screen.getByRole("dialog")).toBeVisible();
    expect(within(screen.getByRole("dialog")).getByLabelText("管理密钥")).toHaveValue("");
  });

  it("keeps busy confirmation modal until the real result succeeds", async () => {
    const user = userEvent.setup();
    let resolve!: () => void;
    const confirm = vi.fn(() => new Promise<void>(done => { resolve = done; }));
    render(<Providers><SecretHarness confirm={confirm} /></Providers>);
    const trigger = screen.getByRole("button", { name: "访问敏感信息" });
    await user.click(trigger);
    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getByLabelText("管理密钥"), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "确认" }));
    expect(within(dialog).getByRole("button", { name: "确认" })).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "取消" })).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "关闭" })).toBeDisabled();
    expect(within(dialog).getByLabelText("管理密钥")).toBeDisabled();
    await user.keyboard("{Escape}");
    expect(dialog).toBeVisible();
    await act(async () => resolve());
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(trigger).toHaveFocus());
    expect(confirm).toHaveBeenCalledOnce();
  });
});
