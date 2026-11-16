import { render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { NeuroProvider } from "@neuro/ui";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialPruneDialog } from "./CredentialPruneDialog";
import type { CredentialPruneReview } from "./useCredentialPruneReview";

const review: CredentialPruneReview = {
  providerId: "pool", preview: {
    providerId: "pool", revision: "r1", digest: "digest", archiveRequired: true,
    items: [{ credentialId: "a", classification: "permanent_auth_failure" }, { credentialId: "b", classification: "account_deleted" }]
  },
  receipt: null, error: null, applying: false, loading: false,
};

it("never applies on open; submits only the explicit selection", async () => {
  const user = userEvent.setup();
  const apply = vi.fn(async () => { });
  render(<NeuroProvider><UiLocaleProvider><CredentialPruneDialog review={review} locked={false} onClose={vi.fn()} onApply={apply} /></UiLocaleProvider></NeuroProvider>);
  expect(apply).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "归档并移除所选 (0)" })).toBeDisabled();
  await user.click(screen.getByRole("checkbox", { name: "b" }));
  await user.click(screen.getByRole("button", { name: "归档并移除所选 (1)" }));
  expect(apply).toHaveBeenCalledExactlyOnceWith(["b"]);
});

it("keeps cancellation and return focus in the shared modal lifecycle", async () => {
  function Example() {
    const [open, setOpen] = useState(false);
    return <><button onClick={() => setOpen(true)}>检查入口</button><CredentialPruneDialog review={open ? review : null}
      locked={false} onClose={() => setOpen(false)} onApply={async () => { }} /></>;
  }
  render(<NeuroProvider mode="light"><UiLocaleProvider><Example /></UiLocaleProvider></NeuroProvider>);
  const user = userEvent.setup();
  const trigger = screen.getByRole("button", { name: "检查入口" });
  await user.click(trigger);
  const dialog = await screen.findByRole("dialog");
  expect(dialog.closest("[data-neuro-mode]")).toHaveAttribute("data-neuro-mode", "light");
  await user.keyboard("{Escape}");
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(document.body).not.toHaveAttribute("data-scroll-locked");
});

it("shows partial failures and cannot apply a receipt again", () => {
  render(<NeuroProvider><UiLocaleProvider><CredentialPruneDialog review={{
    ...review, receipt: {
      providerId: "pool", sourceRevision: "r1", revision: "r2", outcome: "completed", removedCount: 1,
      items: [{ credentialId: "a", status: "archived_and_removed" }, { credentialId: "b", status: "archive_failed" }],
    }
  }} locked={false} onClose={vi.fn()} onApply={vi.fn()} /></UiLocaleProvider></NeuroProvider>);
  expect(screen.getByText("已归档并移除")).toBeInTheDocument();
  expect(screen.getByText("归档失败，已保留")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /归档并移除所选/ })).not.toBeInTheDocument();
});

it("bounds bulk selection, blocks dirty/applying actions and keeps errors visible", async () => {
  const user = userEvent.setup();
  const close = vi.fn();
  const many = { ...review, preview: { ...review.preview!, items: Array.from({ length: 129 }, (_, i) => ({ credentialId: `id-${i}`, classification: "account_deleted" as const })) } };
  const view = render(<NeuroProvider><UiLocaleProvider><CredentialPruneDialog review={many} locked={false} onClose={close} onApply={vi.fn()} /></UiLocaleProvider></NeuroProvider>);
  await user.click(screen.getByRole("button", { name: "选择前 128 项" }));
  expect(screen.getAllByRole("checkbox").filter(item => (item as HTMLInputElement).checked)).toHaveLength(128);
  expect(screen.getByRole("checkbox", { name: "id-128" })).toBeDisabled();
  view.rerender(<NeuroProvider><UiLocaleProvider><CredentialPruneDialog review={{ ...many, applying: true }} locked onClose={close} onApply={vi.fn()} /></UiLocaleProvider></NeuroProvider>);
  expect(screen.getByRole("button", { name: "归档并移除所选 (128)" })).toHaveAttribute("aria-busy", "true");
  await user.keyboard("{Escape}");
  expect(close).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "关闭" })).toBeDisabled();
  view.rerender(<NeuroProvider><UiLocaleProvider><CredentialPruneDialog review={{ ...many, error: "stale revision" }} locked={false} onClose={close} onApply={vi.fn()} /></UiLocaleProvider></NeuroProvider>);
  expect(screen.getByRole("alert")).toHaveTextContent("stale revision");
  expect(screen.getByRole("button", { name: /归档并移除所选/ })).toBeDisabled();
});

it("uses standard status and compact empty states without inventing a successful commit", () => {
  const onApply = vi.fn();
  const receipt = { providerId: "pool", sourceRevision: "r1", revision: "r2", outcome: "completed" as const, removedCount: 0, items: [] };
  const wrap = (value: CredentialPruneReview) => <NeuroProvider><UiLocaleProvider><CredentialPruneDialog review={value} locked={false} onClose={vi.fn()} onApply={onApply} /></UiLocaleProvider></NeuroProvider>;
  const view = render(wrap({ ...review, loading: true, preview: null }));
  expect(screen.getByRole("status")).toHaveClass("n-notice", "n-tone-info");
  view.rerender(wrap({ ...review, receipt }));
  expect(screen.getByRole("status")).toHaveTextContent("已移除 0 个账号。");
  expect(screen.getByRole("status")).toHaveClass("n-tone-success");
  view.rerender(wrap({ ...review, receipt: { ...receipt, removedCount: null } }));
  expect(screen.getByRole("status")).toHaveTextContent("提交结果未确认，请刷新核对");
  expect(screen.getByRole("status")).toHaveClass("n-tone-warning");
  expect(screen.queryByRole("button", { name: /归档并移除所选/ })).not.toBeInTheDocument();
  view.rerender(wrap({ ...review, preview: { ...review.preview!, items: [] } }));
  expect(screen.getByRole("status")).toHaveClass("n-empty", "n-empty--compact");
  expect(onApply).not.toHaveBeenCalled();
});
