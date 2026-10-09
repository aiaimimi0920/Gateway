import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders } from "./BrowserConsoleApp.render-fixture";
import { fixture, Harness, key } from "./AccessKeysSection.fixtures";

describe("Key lifecycle", () => {
  beforeEach(() => window.localStorage.clear());

  it("disables and enables the same key without revocation or rotation", async () => {
    const { api, catalog } = fixture([key("same-key")]);
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("same-key");
    expect(
      screen.queryByRole("button", { name: "吊销" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "停用" }));
    await user.click(screen.getByRole("button", { name: "确认停用" }));
    await user.click(await screen.findByRole("button", { name: "启用" }));
    await user.click(screen.getByRole("button", { name: "确认启用" }));
    await waitFor(() =>
      expect(api.setAccessKeyEnabled).toHaveBeenLastCalledWith(
        "management-fixture",
        "same-key",
        true,
      ),
    );
    expect(catalog.accessKeys[0].id).toBe("same-key");
    expect(catalog.accessKeys[0].revokedAt).toBeNull();
    expect(api.rotateAccessKey).not.toHaveBeenCalled();
  });

  it("requires explicit deletion confirmation and removes only the selected row", async () => {
    const { api } = fixture([key("delete-me"), key("keep-me")]);
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    const row = (await screen.findByText("delete-me")).closest("tr")!;
    await user.click(within(row).getByRole("button", { name: "删除" }));
    expect(screen.getByRole("dialog")).toHaveTextContent("历史现金账单保留");
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(api.deleteAccessKey).not.toHaveBeenCalled();
    await user.click(within(row).getByRole("button", { name: "删除" }));
    await user.click(screen.getByRole("button", { name: "确认删除" }));
    await waitFor(() =>
      expect(screen.queryByText("delete-me")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("keep-me")).toBeInTheDocument();
    expect(api.deleteAccessKey).toHaveBeenCalledWith(
      "management-fixture",
      "delete-me",
    );
  });

  it("keeps failed deletion visible and leaves the key intact", async () => {
    const { api } = fixture([key("failed-key")]);
    vi.mocked(api.deleteAccessKey!).mockRejectedValue(
      new Error("delete rejected"),
    );
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("failed-key");
    await user.click(screen.getByRole("button", { name: "删除" }));
    await user.click(screen.getByRole("button", { name: "确认删除" }));
    expect(
      await within(screen.getByRole("dialog")).findByRole("alert"),
    ).toHaveTextContent("delete rejected");
    expect(screen.getByText("failed-key")).toBeInTheDocument();
  });

  it("cannot revive revoked keys and locks destructive actions in read-only mode", async () => {
    const { api } = fixture([key("revoked", { status: "revoked" })]);
    const view = renderWithProviders(<Harness api={api} />);
    await screen.findByText("revoked");
    expect(screen.getByRole("button", { name: "停用" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "删除" })).toBeEnabled();
    view.unmount();
    renderWithProviders(<Harness api={api} locked />);
    await screen.findByText("revoked");
    expect(screen.getByRole("button", { name: "删除" })).toBeDisabled();
  });
});
