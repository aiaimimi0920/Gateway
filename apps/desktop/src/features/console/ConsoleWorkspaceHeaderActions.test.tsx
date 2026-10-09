import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

import { ConsoleWorkspaceHeaderActions } from "./ConsoleWorkspaceHeaderActions";

afterEach(cleanup);

const props = {
  activeWorkspace: "models",
  actionBusy: null,
  autosavePending: false,
  draftDirty: false,
  editorLocked: false,
  accountLedgerProviderOptions: [],
  modelPoolDialogProviderOptions: [{}],
  setProviderCatalogDialogOpen: vi.fn(),
  openAddCredentialDialog: vi.fn(),
  addAccountGroupRow: vi.fn(),
  openAddModelDialog: vi.fn(),
  t: (zh: string, _en: string) => zh,
};

it("omits the saved label and its status container without removing the add action", async () => {
  const onAdd = vi.fn();
  render(<ConsoleWorkspaceHeaderActions {...props} openAddModelDialog={onAdd} />);
  expect(screen.queryByText("已自动保存")).not.toBeInTheDocument();
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
  await userEvent.setup().click(screen.getByRole("button", { name: "添加模型" }));
  expect(onAdd).toHaveBeenCalledOnce();
});

it("preserves saving, pending and unapplied-change feedback", () => {
  const view = render(<ConsoleWorkspaceHeaderActions {...props} actionBusy="save" />);
  expect(screen.getByRole("status")).toHaveTextContent("自动保存中...");
  view.rerender(<ConsoleWorkspaceHeaderActions {...props} autosavePending />);
  expect(screen.getByRole("status")).toHaveTextContent("待自动保存");
  view.rerender(<ConsoleWorkspaceHeaderActions {...props} draftDirty />);
  expect(screen.getByRole("status")).toHaveTextContent("更改未应用");
  view.rerender(<ConsoleWorkspaceHeaderActions {...props} />);
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
});
