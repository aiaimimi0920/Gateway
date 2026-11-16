import { useState } from "react";
import { createRequire } from "node:module";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { ProviderRemoveDialog } from "./ProviderRemoveDialog";
import { pilotSection } from "./AccountsLedgerWorkspace.fixtures";
import type { AccountsLedgerPilotSection } from "./accountsLedgerTypes";

it("uses the application's React and React DOM instances", () => {
  const app = createRequire(import.meta.url);
  const ui = createRequire(new URL(import.meta.resolve("@neuro/ui")));
  expect(ui.resolve("react")).toBe(app.resolve("react"));
  expect(ui.resolve("react-dom")).toBe(app.resolve("react-dom"));
});

it("keeps a locked removal cancellable and returns focus to its real trigger", async () => {
  const confirm = vi.fn();
  function Example() {
    const [pending, setPending] = useState<AccountsLedgerPilotSection | null>(null);
    return <><button onClick={() => setPending(pilotSection())}>删除入口</button>
      <ProviderRemoveDialog pending={pending} locked onCancel={() => setPending(null)} onConfirm={confirm} t={zh => zh} /></>;
  }
  render(<NeuroProvider><Example /></NeuroProvider>);
  const user = userEvent.setup();
  const trigger = screen.getByRole("button", { name: "删除入口" });
  await user.click(trigger);
  expect(await screen.findByRole("alertdialog")).toHaveAccessibleName("确认删除凭据池");
  expect(screen.getByRole("button", { name: "确认删除凭据池" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "取消" })).toBeEnabled();
  await user.keyboard("{Escape}");
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(confirm).not.toHaveBeenCalled();
});

it("does not dismiss or report success before the application confirms completion", async () => {
  const confirm = vi.fn();
  render(<NeuroProvider><ProviderRemoveDialog pending={pilotSection()} locked={false}
    onCancel={() => {}} onConfirm={confirm} t={zh => zh} /></NeuroProvider>);
  await userEvent.click(await screen.findByRole("button", { name: "确认删除凭据池" }));
  expect(confirm).toHaveBeenCalledOnce();
  expect(screen.getByRole("alertdialog")).toBeVisible();
  expect(screen.getByText(/不会删除磁盘上的凭据文件/)).toBeVisible();
});
