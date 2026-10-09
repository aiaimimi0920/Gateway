import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AccountBillingDialog } from "./AccountBillingDialog";
import { AccountBillingAction } from "./AccountBillingAction";
import { deferred } from "./BrowserConsoleApp.render-fixture";

const state = vi.hoisted(() => ({
  api: { pricing: vi.fn(), saveMultiplier: vi.fn() },
  token: "admin" as string | null,
  sessionBusy: false,
}));
vi.mock("./useCashBillingApi", () => ({ useCashBillingApi: () => state }));
const t = (zh: string) => zh;
const props = {
  providerId: "p",
  accountId: "a",
  name: "Account A",
  locked: false,
  onClose: vi.fn(),
  t,
};
beforeEach(() => {
  vi.clearAllMocks();
  state.token = "admin";
  state.api.pricing.mockResolvedValue({
    accountBillingMultipliers: { a: "0.800000", b: "2" },
  });
  state.api.saveMultiplier.mockResolvedValue({ providerAccount: { id: "p" } });
});
describe("account billing editor", () => {
  it("loads the real account and saves a string without changing other accounts", async () => {
    const user = userEvent.setup();
    render(<AccountBillingDialog {...props} />);
    const field = await screen.findByRole("textbox", { name: "账户倍率" });
    await waitFor(() => expect(field).toHaveValue("0.800000"));
    await user.clear(field);
    await user.type(field, "0");
    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(state.api.saveMultiplier).toHaveBeenCalledWith(
      "admin",
      "p",
      "a",
      "0",
      expect.any(AbortSignal),
    );
    expect(props.onClose).toHaveBeenCalledOnce();
  });
  it("rejects invalid precision and treats blank as explicit reset", async () => {
    const user = userEvent.setup();
    render(<AccountBillingDialog {...props} />);
    const field = screen.getByRole("textbox");
    await waitFor(() => expect(field).toBeEnabled());
    await user.clear(field);
    await user.type(field, "0.0000001");
    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("最多六位小数");
    expect(state.api.saveMultiplier).not.toHaveBeenCalled();
    await user.clear(field);
    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(state.api.saveMultiplier).toHaveBeenCalledWith(
      "admin",
      "p",
      "a",
      null,
      expect.any(AbortSignal),
    );
  });
  it("requires re-reading after an uncertain write and never silently succeeds", async () => {
    state.api.saveMultiplier.mockRejectedValue(new Error("network"));
    const user = userEvent.setup();
    render(<AccountBillingDialog {...props} />);
    const save = screen.getByRole("button", { name: "保存" });
    await waitFor(() => expect(save).toBeEnabled());
    await user.click(save);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "未能确认保存结果",
    );
    expect(save).toBeDisabled();
    expect(props.onClose).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "重新读取" }));
    await waitFor(() => expect(save).toBeEnabled());
    expect(state.api.pricing).toHaveBeenCalledTimes(2);
  });
  it("cancels stale reads when the account changes and aborts on unmount", async () => {
    const first = deferred<{
      accountBillingMultipliers: Record<string, string>;
    }>();
    state.api.pricing.mockReturnValueOnce(first.promise);
    const view = render(<AccountBillingDialog {...props} />);
    const signal = state.api.pricing.mock.calls[0][2] as AbortSignal;
    view.rerender(<AccountBillingDialog {...props} accountId="b" />);
    await waitFor(() => expect(screen.getByRole("textbox")).toHaveValue("2"));
    await act(async () =>
      first.resolve({ accountBillingMultipliers: { a: "10" } }),
    );
    expect(screen.getByRole("textbox")).toHaveValue("2");
    expect(signal.aborted).toBe(true);
    view.unmount();
    expect(state.api.pricing.mock.calls[1][2].aborted).toBe(true);
  });
  it("reads only on open and restores keyboard focus after Escape", async () => {
    const user = userEvent.setup();
    render(<AccountBillingAction {...props} />);
    expect(state.api.pricing).not.toHaveBeenCalled();
    const trigger = screen.getByRole("button");
    await user.click(trigger);
    await waitFor(() => expect(screen.getByRole("textbox")).toBeEnabled());
    await user.keyboard("{Escape}");
    await waitFor(() => expect(trigger).toHaveFocus());
  });
});
