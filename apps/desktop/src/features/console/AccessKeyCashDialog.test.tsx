import { act, render, screen, waitFor, within } from "@testing-library/react";
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
    const unknown = (await screen.findByText("待核对")).closest("tr")!;
    expect(within(unknown).getAllByRole("cell")[3]).toHaveTextContent("—");
    const released = screen.getByText("已释放").closest("tr")!;
    expect(within(released).getAllByRole("cell")[3]).toHaveTextContent(/^0$/);
    expect(screen.getByText("已预占")).toBeInTheDocument();
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
});
