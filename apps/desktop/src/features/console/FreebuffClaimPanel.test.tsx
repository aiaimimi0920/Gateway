import { act, fireEvent, render as renderBase, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { expect, it, vi } from "vitest";
import { FreebuffCatalogPanel, type FreebuffCatalogProps } from "./FreebuffCatalogPanel";
import { freebuffClaimEnd, freebuffClaimJournal } from "./freebuffClaim";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const render = (ui: Parameters<typeof renderBase>[0]) => renderBase(ui, { wrapper: NeuroProvider });
const attempt = "e52ca336-8d50-4654-9d0d-f1da09b34749";
function owner() {
  const value = { revision: "r1", providerId: "pool", credentialId: "b", businessOrigin: "https://codebuff.com", client: "cli",
    cursor: null, claim: { attemptId: attempt, modelKey: "removed-model" }, history: [], nextCursor: null };
  const request = vi.fn().mockResolvedValue(value);
  const props: FreebuffCatalogProps & { savedModelKeys: string[]; savedBusiness: { origin: "https://codebuff.com"; client: "cli" } } = {
    client: { request }, providerId: "pool", credentialId: "b", expectedRevision: "r1", managementToken: "owner", secretGrant: "grant",
    disabled: false, onRequestSecretAccess: vi.fn(), savedModelKeys: [], savedBusiness: { origin: "https://codebuff.com", client: "cli" },
  };
  return { props, request, value };
}
it("reads local original claims without a selected current model or any automatic request", async () => {
  const { props, request } = owner(); render(<FreebuffCatalogPanel {...props} />);
  expect(request).not.toHaveBeenCalled();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "检查本地会话" })); });
  expect(request).toHaveBeenCalledExactlyOnceWith("/v1/internal/gateway/console/freebuff-session-claim", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner", secretGrant: "grant", signal: expect.any(AbortSignal),
    body: { providerId: "pool", credentialId: "b", expectedRevision: "r1", cursor: null },
  }));
  expect(screen.getByText("removed-model")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "结束原会话" })).toBeEnabled();
});
const receipt = { refund: 0, refundSources: null, refundPending: false, inGrace: false };
const run = { phase: "finish_accepted", runIdRecorded: true, totals: { totalSteps: 1, directCredits: 13, totalCredits: 13 },
  lastStep: { cost: 0.125, upstreamEstimate: null, sdkCredits: 13 }, lastError: null, finishStatus: "completed", finishHttpStatus: 200 };
it("shows durable run facts without treating FINISH as settlement or offering replay", async () => {
  const { props, request, value } = owner(); request.mockResolvedValue({ ...value, claim: { ...value.claim, run } });
  render(<FreebuffCatalogPanel {...props} />); await read();
  expect(screen.getByText(/FINISH 已接受（未结算）/)).toBeInTheDocument();
  expect(screen.getByText(/已报告 SDK credits：13/)).toBeInTheDocument();
  expect(request).toHaveBeenCalledTimes(1); expect(screen.queryByRole("button", { name: /重试|续跑|重放/ })).not.toBeInTheDocument();
});
it("keeps known prior-step fees separate from an unknown current model send", async () => {
  const { props, request, value } = owner();
  request.mockResolvedValue({ ...value, claim: { ...value.claim, run: { ...run, phase: "model_pending", totals: null,
    priorTotals: run.totals, lastStep: null, finishStatus: null, finishHttpStatus: null } } });
  render(<FreebuffCatalogPanel {...props} />); await read();
  expect(screen.getByText(/此前已报告 SDK credits：13/)).toBeInTheDocument();
  expect(screen.getByText(/模型发送结果未确认/)).toHaveTextContent("已报告 SDK credits：—");
  expect(request).toHaveBeenCalledTimes(1);
});
it.each([{ runId: "private-run" }, { clientId: "private-client" }, { sealed: "private" }, { totals: { totalSteps: 1, directCredits: 0, totalCredits: 13 } },
  { phase: "model_pending" }, { finishHttpStatus: null }, { runIdRecorded: false }])("rejects secret-bearing or inconsistent durable run facts: %j", patch => {
  const { value } = owner();
  expect(freebuffClaimJournal.safeParse({ ...value, claim: { ...value.claim, run: { ...run, ...patch } } }).success).toBe(false);
});
function end(value: ReturnType<typeof owner>["value"], extra = {}) {
  const { cursor: _cursor, history: _history, nextCursor: _next, claim, ...identity } = value;
  return { ...identity, attemptId: attempt, modelKey: claim.modelKey, receipt, localReleased: true, fromHistory: false, ...extra };
}
async function read() { await act(async () => { fireEvent.click(screen.getByRole("button", { name: "检查本地会话" })); }); }
async function confirm() { fireEvent.click(screen.getByRole("button", { name: "结束原会话" }));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "确认结束" })); }); }
it("requires a fresh exact-attempt confirmation and never auto-replays or saves the account", async () => {
  const { props, request, value } = owner(); request.mockResolvedValueOnce(value).mockResolvedValueOnce(end(value));
  render(<FreebuffCatalogPanel {...props} />); await read();
  fireEvent.click(screen.getByRole("button", { name: "结束原会话" })); expect(request).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("alertdialog")).toHaveTextContent(attempt);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "确认结束" })); });
  expect(request.mock.calls[1][0]).toBe("/v1/internal/gateway/console/freebuff-session-claim/end");
  expect(request.mock.calls[1][2].body).toEqual({ providerId: "pool", credentialId: "b", expectedRevision: "r1", attemptId: attempt });
  expect(screen.getByText("原会话已收尾，最终退款：0 Freebucks")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "结束原会话" })).not.toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(2);
});
it.each([{ refund: null, refundPending: false, inGrace: false }, { refund: 0, refundPending: true, inGrace: false },
  { refund: 0, refundPending: false, inGrace: true }])("retains a non-final end without inventing a refund or replay: %j", async patch => {
  const { props, request, value } = owner(); request.mockResolvedValueOnce(value).mockResolvedValueOnce(end(value, { receipt: { ...receipt, ...patch }, localReleased: false }));
  render(<FreebuffCatalogPanel {...props} />); await read(); await confirm();
  expect(screen.getByText("结束结果尚未最终确认，原记录已保留。")).toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(2);
});
it.each([{ providerId: "other" }, { credentialId: "other" }, { expectedRevision: "r2" }, { managementToken: "other" },
  { secretGrant: null }, { disabled: true }, { savedBusiness: { origin: "https://www.codebuff.com" as const, client: "cli" as const } }])(
  "invalidates original-attempt confirmation on context changes: %j", async patch => {
    const { props, request } = owner(); const view = render(<FreebuffCatalogPanel {...props} />); await read();
    fireEvent.click(screen.getByRole("button", { name: "结束原会话" })); view.rerender(<FreebuffCatalogPanel {...props} {...patch} />);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(1);
  });
it("stops waiting without claiming rollback, ignores late end and requires another explicit read", async () => {
  const { props, request, value } = owner(); let resolve!: (value: unknown) => void;
  request.mockResolvedValueOnce(value).mockImplementationOnce(() => new Promise(done => { resolve = done; }));
  const view = render(<FreebuffCatalogPanel {...props} />); await read(); await confirm();
  const signal = request.mock.calls[1][2].signal; expect(screen.getByRole("button", { name: "读取目录" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "停止会话操作等待" })); expect(signal.aborted).toBe(true);
  await act(async () => { resolve(end(value)); }); expect(screen.queryByText(/原会话已收尾/)).not.toBeInTheDocument();
  expect(screen.getByText(/已停止等待，请重新检查本地会话/)).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "结束原会话" })).not.toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(2); view.unmount();
});
it("does not replay after grant restoration or borrow an unsaved business target", async () => {
  const { props, request } = owner(); const view = render(<FreebuffCatalogPanel {...props} secretGrant={null} />); await read();
  expect(props.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(request).not.toHaveBeenCalled();
  view.rerender(<FreebuffCatalogPanel {...props} />); expect(request).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("客户端"), { target: { value: "desktop" } });
  expect(screen.getByRole("button", { name: "检查本地会话" })).toBeDisabled();
});
it("drops late journal results after context change or unmount", async () => {
  const { props, request, value } = owner(); let resolve!: (value: unknown) => void;
  request.mockImplementation(() => new Promise(done => { resolve = done; })); const view = render(<FreebuffCatalogPanel {...props} />); await read();
  view.rerender(<FreebuffCatalogPanel {...props} expectedRevision="r2" />); expect(request.mock.calls[0][2].signal.aborted).toBe(true);
  await act(async () => { resolve(value); }); expect(screen.queryByText("removed-model")).not.toBeInTheDocument();
  await read(); view.unmount(); expect(request.mock.calls[1][2].signal.aborted).toBe(true);
});
it.each(["revision", "providerId", "credentialId", "businessOrigin", "client"])("rejects foreign journal identity: %s", async field => {
  const { props, request, value } = owner(); request.mockResolvedValue({ ...value, [field]: "foreign" }); render(<FreebuffCatalogPanel {...props} />); await read();
  expect(screen.getByText(/本地会话读取失败/)).toBeInTheDocument(); expect(screen.queryByRole("button", { name: "结束原会话" })).not.toBeInTheDocument();
});
it("rejects secret-bearing and inconsistent journal or end facts instead of inventing success", () => {
  const { value } = owner();
  for (const patch of [{ handle: "secret" }, { claim: { ...value.claim, sealed: "secret" } }, { history: [{ attemptId: attempt, modelKey: "old", receipt: { ...receipt, refund: null } }] }]) {
    expect(freebuffClaimJournal.safeParse({ ...value, ...patch }).success).toBe(false);
  }
  for (const patch of [{ localReleased: false }, { receipt: { ...receipt, refund: -1 } }, { receipt: { ...receipt, refund: null } },
    { receipt: { ...receipt, refundPending: true } }, { authToken: "secret" }]) expect(freebuffClaimEnd.safeParse(end(value, patch)).success).toBe(false);
});
it("pages bounded history using only the returned cursor and keeps the original claim", async () => {
  const { props, request, value } = owner();
  const history = Array.from({ length: 20 }, (_, n) => ({ attemptId: `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`, modelKey: `old-${n}`, receipt }));
  const cursor = history.at(-1)!.attemptId;
  request.mockResolvedValueOnce({ ...value, history, nextCursor: cursor }).mockResolvedValueOnce({ ...value, cursor });
  render(<FreebuffCatalogPanel {...props} />); await read();
  expect(screen.getByRole("list", { name: "结束记录" }).children).toHaveLength(20);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "更早的结束记录" })); });
  expect(request.mock.calls[1][2].body).toEqual({ providerId: "pool", credentialId: "b", expectedRevision: "r1", cursor });
  expect(screen.getByText("removed-model")).toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(2);
});
it.each(["attemptId", "modelKey", "revision", "credentialId"])("refuses mismatched end identity without releasing or replaying: %s", async field => {
  const { props, request, value } = owner();
  request.mockResolvedValueOnce(value).mockResolvedValueOnce(end(value, { [field]: field === "attemptId" ? "00000000-0000-4000-8000-000000000001" : "foreign" }));
  render(<FreebuffCatalogPanel {...props} />); await read(); await confirm();
  expect(screen.getByText(/结束结果尚未确认，请重新检查/)).toBeInTheDocument();
  expect(screen.queryByText(/原会话已收尾/)).not.toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(2);
});
it("hides upstream errors and consumes failed confirmation until another explicit journal read", async () => {
  const { props, request, value } = owner(); request.mockResolvedValueOnce(value).mockRejectedValueOnce(new Error("synthetic-private-token"));
  render(<FreebuffCatalogPanel {...props} />); await read(); await confirm();
  expect(screen.getByText(/结束结果尚未确认，请重新检查/)).toBeInTheDocument(); expect(screen.queryByText(/synthetic-private-token/)).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "结束原会话" })).not.toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(2);
});
