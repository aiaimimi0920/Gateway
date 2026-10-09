import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { renderWithProviders } from "./BrowserConsoleApp.render-fixture";
import {
  fixture,
  Harness,
  key,
  openCreate,
} from "./AccessKeysSection.fixtures";

describe("cash-limited API Key editor", () => {
  it("creates a USD limit while retaining count and token quota choices", async () => {
    const { api, catalog } = fixture();
    catalog.cashQuotaSupported = true;
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    const dialog = await openCreate(user);
    await user.type(
      within(dialog).getByRole("textbox", { name: "名称" }),
      "cash key",
    );
    const quota = within(dialog).getByRole("combobox", { name: "额度限制" });
    expect(
      within(quota).getByRole("option", { name: "Token 总量" }),
    ).toBeInTheDocument();
    expect(
      within(quota).getByRole("option", { name: "请求次数" }),
    ).toBeInTheDocument();
    await user.selectOptions(quota, "cash_prepaid");
    const create = within(dialog).getByRole("button", { name: "创建 API Key" });
    expect(create).toBeDisabled();
    await user.type(
      within(dialog).getByRole("spinbutton", { name: "总花费（USD）" }),
      "2.40",
    );
    await user.click(create);
    await screen.findByRole("dialog", { name: "API Key 已生成" });
    expect(api.createAccessKey).toHaveBeenCalledWith(
      "management-fixture",
      expect.objectContaining({
        quota: { mode: "cash_prepaid", limit: 2_400_000, currency: "USD" },
      }),
    );
  });

  it("edits only the cap, shows settled/reserved/available and restores focus", async () => {
    const accessKey = key("cash key", {
      metadata: { accountGroupIds: ["group-a"] },
    });
    const { api, catalog } = fixture([accessKey]);
    catalog.cashQuotaSupported = true;
    catalog.balances = [
      {
        accessKeyId: accessKey.id,
        balanceMode: "cash_prepaid",
        status: "active",
        totalTokens: null,
        remainingTokens: null,
        totalMessages: null,
        remainingMessages: null,
        unlimitedUntil: null,
        periodStartsAt: null,
        periodEndsAt: null,
        updatedAt: "2026-10-08T00:00:00Z",
        cash: {
          currency: "USD",
          totalMicros: 3_000_000,
          spentMicros: 2_400_000,
          reservedMicros: 500_000,
          pendingRequests: 1,
        },
      },
    ];
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("cash key");
    expect(screen.getByText("0.1 USD")).toBeInTheDocument();
    const trigger = screen.getByRole("button", { name: "编辑" });
    await user.click(trigger);
    const dialog = screen.getByRole("dialog", { name: "编辑 API Key" });
    expect(
      within(dialog).getByText("已结算 2.4 · 预占 0.5 · 可用 0.1 USD"),
    ).toBeInTheDocument();
    const limit = within(dialog).getByRole("spinbutton", {
      name: "总花费（USD）",
    });
    await user.clear(limit);
    await user.type(limit, "5.000001");
    await user.click(within(dialog).getByRole("button", { name: "保存" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(trigger).toHaveFocus();
    expect(api.updateAccessKey).toHaveBeenCalledWith(
      "management-fixture",
      accessKey.id,
      expect.objectContaining({
        quota: { mode: "cash_prepaid", limit: 5_000_001, currency: "USD" },
      }),
    );
  });

  it("does not reinterpret token totals as dollars or advertise an unsupported backend", async () => {
    const { api, catalog } = fixture();
    const user = userEvent.setup();
    const view = renderWithProviders(<Harness api={api} />);
    let dialog = await openCreate(user);
    expect(
      within(dialog).queryByRole("option", { name: "总花费" }),
    ).not.toBeInTheDocument();
    view.unmount();
    catalog.cashQuotaSupported = true;
    renderWithProviders(<Harness api={api} />);
    dialog = await openCreate(user);
    const quota = within(dialog).getByRole("combobox", { name: "额度限制" });
    await user.selectOptions(quota, "token_prepaid");
    await user.type(
      within(dialog).getByRole("spinbutton", { name: "总 Token 额度" }),
      "1000000",
    );
    await user.selectOptions(quota, "cash_prepaid");
    expect(
      within(dialog).getByRole("spinbutton", { name: "总花费（USD）" }),
    ).toHaveValue(null);
    await user.type(
      within(dialog).getByRole("spinbutton", { name: "总花费（USD）" }),
      "0.0000001",
    );
    expect(
      within(dialog).getByRole("button", { name: "创建 API Key" }),
    ).toBeDisabled();
  });
});
