import { act, render as renderUi, screen, waitFor, within } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import type { ReactNode } from "react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AccessKeyCashDialog } from "./AccessKeyCashDialog";
import { key } from "./AccessKeysSection.fixtures";
import { deferred } from "./BrowserConsoleApp.render-fixture";
import type { CashReceipt } from "./cashBillingApi";

const state = vi.hoisted(() => ({
  api: { ledger: vi.fn() },
  token: "admin" as string | null,
}));
vi.mock("./useCashBillingApi", () => ({ useCashBillingApi: () => state }));
const t = (zh: string) => zh;
const render = (ui: ReactNode) => renderUi(ui, { wrapper: ({ children }) => <NeuroProvider>{children}</NeuroProvider> });
const quote = {
  providerAccountId: "p",
  credentialId: "a",
  model: "frozen-model",
  groupId: "group-a",
  priceSource: "configured",
  promptMicrosPer1kTokens: 2000,
  completionMicrosPer1kTokens: 10000,
  groupMultiplierPpm: 1_500_000,
  accountMultiplierPpm: 800_000,
  cacheTokensSeparate: false,
};
const receipt: CashReceipt = {
  requestId: "r1",
  accessKeyId: "old-rotated-key",
  createdAt: "2026-10-08T00:00:00Z",
  status: "settled",
  currency: "USD",
  reservedMicros: 500_000,
  amountMicros: 123_456,
  quotes: [quote],
  settledQuote: quote,
  reason: null,
  usage: {
    prompt_tokens: 100,
    completion_tokens: 50,
    total_tokens: 150,
    cache_creation_input_tokens: null,
    cache_read_input_tokens: 20,
  },
};
const props = { accessKey: key("Cash Key"), onClose: vi.fn(), t };
beforeEach(() => {
  vi.clearAllMocks();
  state.token = "admin";
  state.api.ledger.mockResolvedValue([receipt]);
});

describe("Key cash bills", () => {
  it("paginates recent bills and resets the page when filtering status", async () => {
    state.api.ledger.mockResolvedValue(Array.from({ length: 25 }, (_, i) => ({
      ...receipt, requestId: `request-${i}`, status: i === 24 ? "released" : "settled",
    })));
    const user = userEvent.setup();
    render(<AccessKeyCashDialog {...props} />);
    const table = within(await screen.findByRole("table"));
    expect(table.getAllByRole("row")).toHaveLength(11);
    const pager = within(screen.getByRole("navigation", { name: "账单分页" }));
    await user.click(pager.getByRole("button", { name: "下一页" }));
    expect(pager.getByText("2 / 3")).toBeInTheDocument();
    await user.selectOptions(screen.getByRole("combobox", { name: "账单状态" }), "released");
    expect(table.getAllByRole("row")).toHaveLength(2);
    expect(table.getByText("已释放")).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "账单分页" })).not.toBeInTheDocument();
  });
  it("shows frozen actual amounts and old-key IDs without recomputing from the tariff", async () => {
    const user = userEvent.setup();
    render(<AccessKeyCashDialog {...props} />);
    expect(await screen.findByText("0.123456")).toBeInTheDocument();
    await user.click(screen.getByText("用量与固定报价"));
    expect(screen.getByText("old-rotated-key")).toBeInTheDocument();
    expect(screen.getByText("frozen-model")).toBeInTheDocument();
    expect(screen.getByText("1.5 / 0.8")).toBeInTheDocument();
    expect(screen.getByText("100 / 50")).toBeInTheDocument();
    expect(screen.getByText("— / 20")).toBeInTheDocument();
  });
  it("keeps unknown settlement distinct from zero and displays all lifecycle states", async () => {
    state.api.ledger.mockResolvedValue([
      {
        ...receipt,
        status: "unresolved",
        amountMicros: null,
        usage: null,
        settledQuote: null,
        reason: "usage_unavailable",
      },
      { ...receipt, requestId: "r2", status: "released", amountMicros: 0 },
      { ...receipt, requestId: "r3", status: "reserved", amountMicros: null },
    ]);
    render(<AccessKeyCashDialog {...props} />);
    const unknown = (await screen.findByText("待核对", { selector: "td span" })).closest("tr")!;
    expect(within(unknown).getAllByRole("cell")[2]).toHaveTextContent(/^—$/);
    const released = screen.getByText("已释放", { selector: "td span" }).closest("tr")!;
    expect(within(released).getAllByRole("cell")[2]).toHaveTextContent(/^0$/);
    expect(screen.getByText("已预占", { selector: "td span" })).toBeInTheDocument();
  });
  it("distinguishes loading, failure and a successfully read empty ledger", async () => {
    const pending = deferred<CashReceipt[]>();
    state.api.ledger.mockReturnValueOnce(pending.promise);
    const user = userEvent.setup();
    render(<AccessKeyCashDialog {...props} />);
    expect(screen.getByRole("status")).toHaveTextContent("读取中");
    await act(async () => pending.reject(new Error("network")));
    expect(screen.getByRole("alert")).toHaveTextContent("账单读取失败");
    expect(screen.queryByText("暂无现金账单记录")).not.toBeInTheDocument();
    state.api.ledger.mockResolvedValueOnce([]);
    await user.click(screen.getByRole("button", { name: "重试" }));
    expect(await screen.findByText("暂无现金账单记录")).toBeInTheDocument();
  });
  it("aborts old requests and clears financial data when authorization is lost", async () => {
    const view = render(<AccessKeyCashDialog {...props} />);
    await screen.findByText("0.123456");
    const signal = state.api.ledger.mock.calls[0][2] as AbortSignal;
    state.token = null;
    view.rerender(<AccessKeyCashDialog {...props} />);
    await waitFor(() =>
      expect(screen.queryByText("0.123456")).not.toBeInTheDocument(),
    );
    expect(signal.aborted).toBe(true);
    expect(state.api.ledger).toHaveBeenCalledOnce();
    expect(screen.getByRole("alert")).toHaveTextContent("请先登录");
  });
  it("uses standard rows, status and disclosures without refetching or replacing retained receipts", async () => {
    const user = userEvent.setup();
    render(<AccessKeyCashDialog {...props} />);
    const table = await screen.findByRole("table", { name: "最近 100 条账单" });
    expect(table).toHaveClass("n-table");
    expect(table.querySelectorAll('thead th[scope="col"]')).toHaveLength(4);
    const row = table.querySelector<HTMLTableRowElement>('[data-row-key="r1"]')!;
    expect(within(row).getByRole("rowheader")).toBeInTheDocument();
    expect(within(row).getByText("已结算")).toHaveClass("n-badge", "n-tone-success");
    const details = row.querySelector("details")!;
    const summary = details.querySelector("summary")!;
    expect(details).toHaveClass("n-disclosure");
    await user.click(summary);
    expect(details).toHaveAttribute("open");
    await user.selectOptions(screen.getByRole("combobox", { name: "账单状态" }), "settled");
    expect(table.querySelector('[data-row-key="r1"]')).toBe(row);
    expect(row.querySelector("summary")).toBe(summary);
    expect(details).toHaveAttribute("open");
    expect(state.api.ledger).toHaveBeenCalledOnce();
    expect(document.querySelector(".nt-key-ledger, .nt-key-bill-status")).toBeNull();
  });
  it("preserves the empty-filter status and the recent-history boundary", async () => {
    const user = userEvent.setup();
    render(<AccessKeyCashDialog {...props} />);
    await screen.findByRole("table");
    await user.selectOptions(screen.getByRole("combobox", { name: "账单状态" }), "released");
    expect(screen.getByRole("status")).toHaveTextContent("没有此状态的账单");
    expect(screen.getByText("没有此状态的账单").closest(".n-empty")).toBeInTheDocument();
    expect(screen.getByText("仅展示最近 100 条记录，不代表全部历史账单。")).toBeInTheDocument();
    expect(state.api.ledger).toHaveBeenCalledOnce();
  });
  it("aborts a pending read on close and ignores its late result", async () => {
    const pending = deferred<CashReceipt[]>();
    state.api.ledger.mockReturnValueOnce(pending.promise);
    const view = render(<AccessKeyCashDialog {...props} />);
    const signal = state.api.ledger.mock.calls[0][2] as AbortSignal;
    view.unmount();
    expect(signal.aborted).toBe(true);
    await act(async () => pending.resolve([receipt]));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(document.querySelector(".n-overlay")).toBeNull();
    expect(document.body).not.toHaveAttribute("data-scroll-locked");
  });
});
