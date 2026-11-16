import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NeuroProvider } from "@neuro/ui";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import { ProviderLifecycleActionDialog, type PendingProviderLifecycleAction } from "./ProviderLifecycleActionDialog";

const pending: PendingProviderLifecycleAction = { kind: "prune", providerId: "fixture", providerLabel: "测试池", permanentDeleteEnabled: false, archivedCredentialCount: 7 };
const t = (zh: string) => zh;
const cases = [
  { kind: "prune", permanent: false, title: "确认归档失效号", action: "归档失效号", detail: "移入账号归档目录" },
  { kind: "prune", permanent: true, title: "确认彻底删除失效号", action: "彻底删除失效号", detail: "直接彻底删除" },
  { kind: "enable-permanent-delete", permanent: false, title: "开启彻底删除模式", action: "确认开启", detail: "保存当前草稿后生效" },
  { kind: "purge-archive", permanent: false, title: "确认清空账号归档", action: "彻底删除归档", detail: "7 个账号" },
] as const;

it.each(cases)("keeps $title explicit and leaves completion with the application", async ({ kind, permanent, title, action, detail }) => {
  const confirm = vi.fn();
  const cancel = vi.fn();
  render(<NeuroProvider><ProviderLifecycleActionDialog pending={{ ...pending, kind, permanentDeleteEnabled: permanent }}
    onConfirm={confirm} onCancel={cancel} t={t} /></NeuroProvider>);
  const user = userEvent.setup();
  const dialog = screen.getByRole("alertdialog", { name: title });
  expect(dialog).toHaveAccessibleDescription(expect.stringContaining(detail));
  expect(screen.getByRole("button", { name: "取消" })).toHaveFocus();
  expect(confirm).not.toHaveBeenCalled();
  if (kind === "purge-archive") expect(dialog).toHaveTextContent("云服务的历史版本和保留策略不在本次清空范围内");
  const button = screen.getByRole("button", { name: action });
  expect(button).toHaveClass(kind === "prune" && !permanent ? "n-button--primary" : "n-button--danger");
  await user.click(button);
  expect(confirm).toHaveBeenCalledOnce();
  expect(cancel).not.toHaveBeenCalled();
  expect(dialog).toBeInTheDocument();
});

it("cancels without changing lifecycle policy or invoking deletion", async () => {
  const confirm = vi.fn();
  const cancel = vi.fn();
  render(<NeuroProvider><ProviderLifecycleActionDialog pending={pending} onConfirm={confirm} onCancel={cancel} t={t} /></NeuroProvider>);
  const user = userEvent.setup();
  await user.keyboard("{Escape}");
  expect(cancel).toHaveBeenCalledOnce();
  expect(confirm).not.toHaveBeenCalled();
});

it("returns to the card fallback after the application starts an action and disables its opener", async () => {
  function Example() {
    const [open, setOpen] = useState(false);
    const [disabled, setDisabled] = useState(false);
    return <article><div className="nt-provider-card__back">
      <button className="nt-provider-card__flip">翻面</button>
      <button disabled={disabled} onClick={() => setOpen(true)}>清空归档入口</button>
    </div><ProviderLifecycleActionDialog pending={open ? { ...pending, kind: "purge-archive" } : null} t={t}
      onCancel={() => setOpen(false)} onConfirm={() => { setDisabled(true); setOpen(false); }} /></article>;
  }
  render(<NeuroProvider><Example /></NeuroProvider>);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "清空归档入口" }));
  await user.click(screen.getByRole("button", { name: "彻底删除归档" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "翻面" })).toHaveFocus());
  expect(screen.getByRole("button", { name: "清空归档入口" })).toBeDisabled();
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
});
