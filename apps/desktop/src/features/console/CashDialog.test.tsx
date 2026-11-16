import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NeuroProvider } from "@neuro/ui";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import { CashDialog } from "./CashDialog";

const t = (zh: string) => zh;

it("uses the owning theme, traps keyboard focus and restores the actual opener", async () => {
  function Example() {
    const [open, setOpen] = useState(false);
    return <><button onClick={() => setOpen(true)}>账单入口</button>
      {open ? <CashDialog title="账单" description="Key A" onClose={() => setOpen(false)} t={t}>
        <button>明细</button>
      </CashDialog> : null}</>;
  }
  render(<NeuroProvider mode="light"><Example /></NeuroProvider>);
  const user = userEvent.setup();
  const trigger = screen.getByRole("button", { name: "账单入口" });
  await user.click(trigger);
  const dialog = screen.getByRole("dialog", { name: "账单" });
  expect(dialog.closest("[data-neuro-mode]")).toHaveAttribute("data-neuro-mode", "light");
  expect(dialog).toHaveAccessibleDescription("Key A");
  await user.tab({ shift: true });
  expect(dialog.contains(document.activeElement)).toBe(true);
  await user.keyboard("{Escape}");
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(document.body).not.toHaveAttribute("data-scroll-locked");
});

it("keeps busy dismissal under the shared dialog owner and permits closing after completion", async () => {
  const close = vi.fn();
  const content = (busy: boolean) => <NeuroProvider><CashDialog title="倍率" description="Account A"
    busy={busy} onClose={close} t={t}><p>正在保存</p></CashDialog></NeuroProvider>;
  const view = render(content(true));
  const user = userEvent.setup();
  await user.keyboard("{Escape}");
  fireEvent.pointerDown(document.querySelector(".n-overlay")!, { button: 0, pointerType: "mouse" });
  expect(close).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "关闭" })).toBeDisabled();
  view.rerender(content(false));
  await user.click(screen.getByRole("button", { name: "关闭" }));
  expect(close).toHaveBeenCalledOnce();
});
