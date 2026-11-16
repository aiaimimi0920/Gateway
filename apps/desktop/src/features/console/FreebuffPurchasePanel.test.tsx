import { act, fireEvent, render as renderBase, screen, within } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { expect, it, vi } from "vitest";
import { FreebuffCatalogPanel, type FreebuffCatalogProps } from "./FreebuffCatalogPanel";
import { pendingPurchase, pendingResponse } from "../../api/console/freebuff-purchases.fixtures";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const render = (ui: Parameters<typeof renderBase>[0]) => renderBase(ui, { wrapper: NeuroProvider });
function owner() {
  const request = vi.fn().mockResolvedValue(pendingResponse());
  const props: FreebuffCatalogProps & { savedBusiness: { origin: "https://codebuff.com"; client: "cli" } } = {
    client: { request }, providerId: "pool", credentialId: "stable", expectedRevision: "r1", managementToken: "owner", secretGrant: "grant",
    disabled: false, onRequestSecretAccess: vi.fn(), savedBusiness: { origin: "https://codebuff.com", client: "cli" },
  };
  return { request, props };
}
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
const read = () => click("读取待确认购买");
it("does not read or approve automatically, and opens the full frozen quote before approval", async () => {
  const { request, props } = owner(); render(<FreebuffCatalogPanel {...props} />);
  expect(request).not.toHaveBeenCalled(); await read(); await click("查看并批准"); expect(request).toHaveBeenCalledOnce();
  const dialog = screen.getByRole("alertdialog", { name: "确认本次购买" });
  for (const text of ["会话价格", "总余额", "每日剩余", "钱包余额", "预计钱包支出", "可领取赠额", "Solo 折扣", "配额豁免", "上游免询问标记", "预计领取赠额",
    "50 Freebucks", "150 Freebucks", "30 Freebucks", "120 Freebucks", "20 Freebucks", "10 Freebucks", "2 Freebucks",
    pendingPurchase().requestId, pendingPurchase().requestSha256, pendingPurchase().callerFingerprint]) expect(within(dialog).getByText(text)).toBeInTheDocument();
  expect(dialog).toHaveTextContent("不锁价"); await click("返回");
  expect(request).toHaveBeenCalledOnce(); expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
});
it.each([["查看并批准", "批准此次购买", "approve", "批准决定已交付；购买与生成结果由原请求返回。"],
  ["取消此次购买", "确认取消购买", "cancel", "取消决定已交付。"]])("delivers only the explicit %s decision once", async (review, confirm, decision, notice) => {
  const { request, props } = owner(); request.mockResolvedValueOnce(pendingResponse()).mockResolvedValue({ delivered: true });
  render(<FreebuffCatalogPanel {...props} />); await read(); await click(review); expect(request).toHaveBeenCalledOnce();
  const button = screen.getByRole("button", { name: confirm });
  await act(async () => { fireEvent.click(button); fireEvent.click(button); });
  expect(request).toHaveBeenCalledTimes(2);
  expect(request.mock.calls[1][0]).toBe("/v1/internal/gateway/console/freebuff-pending-purchases/decision");
  expect(request.mock.calls[1][2].body).toEqual({ providerId: "pool", credentialId: "stable", expectedRevision: "r1",
    pendingId: pendingPurchase().pendingId, quoteId: pendingPurchase().quoteId, decision });
  expect(screen.getByText(notice)).toBeInTheDocument(); expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "查看并批准" })).not.toBeInTheDocument();
});
it.each([{ providerId: "foreign" }, { credentialId: "foreign" }, { expectedRevision: "r2" }, { managementToken: "new-owner" },
  { secretGrant: null }, { secretGrant: "new-grant" }, { disabled: true }, { savedBusiness: { origin: "https://www.codebuff.com" as const, client: "cli" as const } }])(
  "invalidates the displayed quote and confirmation on context changes: %j", async patch => {
    const { request, props } = owner(); const view = render(<FreebuffCatalogPanel {...props} />); await read(); await click("查看并批准");
    view.rerender(<FreebuffCatalogPanel {...props} {...patch} />);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(); expect(screen.queryByText("catalog-model")).not.toBeInTheDocument();
    expect(request).toHaveBeenCalledOnce();
  });
it("requires another explicit read after grant restoration and rejects unsaved business changes", async () => {
  const { request, props } = owner(); const view = render(<FreebuffCatalogPanel {...props} secretGrant={null} />); await read();
  expect(props.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(request).not.toHaveBeenCalled();
  view.rerender(<FreebuffCatalogPanel {...props} />); expect(request).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("客户端"), { target: { value: "desktop" } });
  expect(screen.getByRole("button", { name: "读取待确认购买" })).toBeDisabled();
});
it("aborts reads on revision changes and unmount and ignores late responses", async () => {
  const { request, props } = owner(); let resolve!: (value: unknown) => void;
  request.mockImplementation(() => new Promise(done => { resolve = done; }));
  const view = render(<FreebuffCatalogPanel {...props} />); await read(); const signal = request.mock.calls[0][2].signal;
  view.rerender(<FreebuffCatalogPanel {...props} expectedRevision="r2" />); expect(signal.aborted).toBe(true);
  await act(async () => { resolve(pendingResponse()); }); expect(screen.queryByText("catalog-model")).not.toBeInTheDocument();
  await read(); view.unmount(); expect(request.mock.calls[1][2].signal.aborted).toBe(true);
});
it("stops waiting without claiming cancellation or replaying a possibly delivered approval", async () => {
  const { request, props } = owner(); let resolve!: (value: unknown) => void;
  request.mockResolvedValueOnce(pendingResponse()).mockImplementationOnce(() => new Promise(done => { resolve = done; }));
  render(<FreebuffCatalogPanel {...props} />); await read(); await click("查看并批准"); await click("批准此次购买");
  expect(screen.getByRole("button", { name: "读取目录" })).toBeDisabled(); expect(screen.getByRole("button", { name: "检查本地会话" })).toBeDisabled();
  const signal = request.mock.calls[1][2].signal; await click("停止购买确认等待"); expect(signal.aborted).toBe(true);
  await act(async () => { resolve({ delivered: true }); });
  expect(screen.getByText(/已停止等待，决定可能已交付/)).toBeInTheDocument();
  expect(screen.queryByText(/批准决定已交付/)).not.toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(2);
});
it.each([new Error("synthetic-private-token"), { status: 409, code: "freebuff_purchase_quote_expired" }])(
  "consumes failed decisions, hides upstream details and does not retry: %j", async error => {
    const { request, props } = owner(); request.mockResolvedValueOnce(pendingResponse()).mockRejectedValueOnce(error);
    render(<FreebuffCatalogPanel {...props} />); await read(); await click("查看并批准"); await click("批准此次购买");
    expect(screen.getByText(/决定交付结果未确认/)).toBeInTheDocument(); expect(screen.queryByText(/synthetic-private-token/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "查看并批准" })).not.toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(2);
  });
it("clears an open confirmation on explicit refresh and renders absent optional meter facts as unknown", async () => {
  const { request, props } = owner(); const row = pendingPurchase();
  row.quote.meter = { balance: 0, dailyRemaining: 0, walletBalance: 0, sessionPrice: 0 }; row.quote.walletSpend = 0;
  request.mockResolvedValue(pendingResponse([row])); render(<FreebuffCatalogPanel {...props} />); await read(); await click("查看并批准");
  const dialog = screen.getByRole("alertdialog"); expect(within(dialog).getAllByText("未知")).toHaveLength(4);
  await click("返回"); await read(); expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(2);
});
it("rejects a foreign read without exposing rows for a decision", async () => {
  const { request, props } = owner(); request.mockResolvedValue({ ...pendingResponse(), credentialId: "other" });
  render(<FreebuffCatalogPanel {...props} />); await read(); expect(screen.getByText(/待确认购买读取失败/)).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "查看并批准" })).not.toBeInTheDocument(); expect(request).toHaveBeenCalledOnce();
});
