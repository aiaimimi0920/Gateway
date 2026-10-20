import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialPruneDialog } from "./CredentialPruneDialog";
import type { CredentialPruneReview } from "./useCredentialPruneReview";

const review: CredentialPruneReview = {
  providerId: "pool", preview: { providerId: "pool", revision: "r1", digest: "digest", archiveRequired: true,
    items: [{ credentialId: "a", classification: "permanent_auth_failure" }, { credentialId: "b", classification: "account_deleted" }] },
  receipt: null, error: null, applying: false, loading: false,
};

it("never applies on open; submits only the explicit selection", async () => {
  const user = userEvent.setup();
  const apply = vi.fn(async () => {});
  render(<UiLocaleProvider><CredentialPruneDialog review={review} locked={false} onClose={vi.fn()} onApply={apply} /></UiLocaleProvider>);
  expect(apply).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "归档并移除所选 (0)" })).toBeDisabled();
  await user.click(screen.getByRole("checkbox", { name: "b" }));
  await user.click(screen.getByRole("button", { name: "归档并移除所选 (1)" }));
  expect(apply).toHaveBeenCalledExactlyOnceWith(["b"]);
});

it("shows partial failures and cannot apply a receipt again", () => {
  render(<UiLocaleProvider><CredentialPruneDialog review={{ ...review, receipt: {
    providerId: "pool", sourceRevision: "r1", revision: "r2", outcome: "completed", removedCount: 1,
    items: [{ credentialId: "a", status: "archived_and_removed" }, { credentialId: "b", status: "archive_failed" }],
  } }} locked={false} onClose={vi.fn()} onApply={vi.fn()} /></UiLocaleProvider>);
  expect(screen.getByText("已归档并移除")).toBeInTheDocument();
  expect(screen.getByText("归档失败，已保留")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /归档并移除所选/ })).not.toBeInTheDocument();
});

it("bounds bulk selection, blocks dirty/applying actions and keeps errors visible", async () => {
  const user = userEvent.setup();
  const close = vi.fn();
  const many = { ...review, preview: { ...review.preview!, items: Array.from({ length: 129 }, (_, i) => ({ credentialId: `id-${i}`, classification: "account_deleted" as const })) } };
  const view = render(<UiLocaleProvider><CredentialPruneDialog review={many} locked={false} onClose={close} onApply={vi.fn()} /></UiLocaleProvider>);
  await user.click(screen.getByRole("button", { name: "选择前 128 项" }));
  expect(screen.getAllByRole("checkbox").filter(item => (item as HTMLInputElement).checked)).toHaveLength(128);
  expect(screen.getByRole("checkbox", { name: "id-128" })).toBeDisabled();
  view.rerender(<UiLocaleProvider><CredentialPruneDialog review={{ ...many, applying: true }} locked onClose={close} onApply={vi.fn()} /></UiLocaleProvider>);
  await user.keyboard("{Escape}");
  expect(close).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "关闭" })).toBeDisabled();
  view.rerender(<UiLocaleProvider><CredentialPruneDialog review={{ ...many, error: "stale revision" }} locked={false} onClose={close} onApply={vi.fn()} /></UiLocaleProvider>);
  expect(screen.getByRole("alert")).toHaveTextContent("stale revision");
  expect(screen.getByRole("button", { name: /归档并移除所选/ })).toBeDisabled();
});
