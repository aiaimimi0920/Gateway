import { useState } from "react";
import { NeuroProvider } from "@neuro/ui";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { CredentialRemoveDialog } from "./CredentialRemoveDialog";
import type { PendingCredentialRemoval } from "./accountCardTypes";

const removal: PendingCredentialRemoval = { providerId: "pool", accountId: "account-1", displayName: "账号 A" };

it("keeps the draft-only risk and leaves confirmation completion to the application", async () => {
  const confirm = vi.fn();
  render(<NeuroProvider><CredentialRemoveDialog pending={removal} onCancel={vi.fn()} onConfirm={confirm} t={zh => zh} /></NeuroProvider>);
  const dialog = await screen.findByRole("alertdialog", { name: "确认删除账号" });
  expect(dialog).toHaveAccessibleDescription(/账号 A（account-1）。保存草稿后该变更才会生效/);
  expect(screen.getByText("此操作不会删除其他账号。")).toBeVisible();
  expect(screen.getByRole("button", { name: "取消" })).toHaveFocus();
  await userEvent.click(screen.getByRole("button", { name: "确认删除" }));
  expect(confirm).toHaveBeenCalledOnce();
  expect(dialog).toBeVisible();
});

it("cancels without deleting and restores the opening account action", async () => {
  const confirm = vi.fn();
  function Example() {
    const [pending, setPending] = useState<PendingCredentialRemoval | null>(null);
    return <><button onClick={() => setPending(removal)}>删除入口</button>
      <CredentialRemoveDialog pending={pending} onCancel={() => setPending(null)} onConfirm={confirm} t={zh => zh} /></>;
  }
  render(<NeuroProvider><Example /></NeuroProvider>);
  const user = userEvent.setup();
  const trigger = screen.getByRole("button", { name: "删除入口" });
  await user.click(trigger);
  await user.click(screen.getByRole("button", { name: "取消" }));
  await waitFor(() => expect(trigger).toHaveFocus());
  await user.keyboard("{Enter}{Escape}");
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(confirm).not.toHaveBeenCalled();
});
