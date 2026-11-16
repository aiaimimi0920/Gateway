import { act, fireEvent, render as renderBase, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { expect, it, vi } from "vitest";
import { FreebuffCatalogPanel, type FreebuffCatalogProps } from "./FreebuffCatalogPanel";
import { freebuffInspectionResponse } from "./freebuffInspection";
import { savedFreebuffModelKeys } from "./freebuffBusiness";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const render = (ui: Parameters<typeof renderBase>[0]) => renderBase(ui, { wrapper: NeuroProvider });
function owner() {
  const value = { revision: "r1", providerId: "pool", credentialId: "b", businessOrigin: "https://codebuff.com", client: "cli",
    modelKey: "open", observedAt: "2026-11-06T00:00:00Z", session: { status: "none" }, meter: { state: "absent" } };
  const request = vi.fn().mockResolvedValue(value);
  const props: FreebuffCatalogProps & { savedModelKeys: string[]; savedBusiness: { origin: "https://codebuff.com"; client: "cli" } } = {
    client: { request }, providerId: "pool", credentialId: "b", expectedRevision: "r1", managementToken: "owner", secretGrant: "grant",
    disabled: false, onRequestSecretAccess: vi.fn(), savedModelKeys: ["open", "other"], savedBusiness: { origin: "https://codebuff.com", client: "cli" },
  };
  return { props, request, value };
}
it("checks only an explicitly selected saved model without buying or requesting on mount", async () => {
  const { props, request } = owner(); render(<FreebuffCatalogPanel {...props} />);
  expect(request).not.toHaveBeenCalled(); expect(screen.getByRole("button", { name: "检查会话与余额" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("检查模型"), { target: { value: "open" } });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "检查会话与余额" })); });
  expect(request).toHaveBeenCalledExactlyOnceWith("/v1/internal/gateway/console/freebuff-session-inspection", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner", secretGrant: "grant", signal: expect.any(AbortSignal),
    body: { providerId: "pool", credentialId: "b", expectedRevision: "r1", modelKey: "open" },
  }));
  expect(screen.getByText("未提供 Freebucks 余额")).toBeInTheDocument();
});
function selectAndRead() {
  fireEvent.change(screen.getByLabelText("检查模型"), { target: { value: "open" } });
  fireEvent.click(screen.getByRole("button", { name: "检查会话与余额" }));
}
it.each([
  { providerId: "other" }, { credentialId: "other" }, { expectedRevision: "r2" }, { managementToken: "other" },
  { secretGrant: "other" }, { secretGrant: null }, { disabled: true }, { client: { request: vi.fn() } },
  { savedBusiness: { origin: "https://www.codebuff.com" as const, client: "cli" as const } }, { savedModelKeys: ["other"] },
])("aborts and drops a late inspection after saved context changes: %j", async (patch) => {
  const { props, request, value } = owner(); let resolve!: (value: unknown) => void;
  request.mockImplementation(() => new Promise((done) => { resolve = done; }));
  const view = render(<FreebuffCatalogPanel {...props} />);
  await act(async () => { selectAndRead(); }); const signal = request.mock.calls[0][2].signal;
  view.rerender(<FreebuffCatalogPanel {...props} {...patch} />); expect(signal.aborted).toBe(true);
  await act(async () => { resolve(value); }); expect(screen.queryByText("无活动会话")).not.toBeInTheDocument();
});
it("cancels on explicit cancel and unmount, ignoring transport which resolves after cancellation", async () => {
  const { props, request, value } = owner(); let resolve!: (value: unknown) => void;
  request.mockImplementation(() => new Promise((done) => { resolve = done; }));
  const view = render(<FreebuffCatalogPanel {...props} />); await act(async () => { selectAndRead(); });
  fireEvent.click(screen.getByRole("button", { name: "取消检查" })); expect(request.mock.calls[0][2].signal.aborted).toBe(true);
  await act(async () => { resolve(value); }); expect(screen.queryByText("无活动会话")).not.toBeInTheDocument();
  await act(async () => { selectAndRead(); }); view.unmount(); expect(request.mock.calls[1][2].signal.aborted).toBe(true);
});
it("does not borrow unsaved catalog target and requests grant restoration without a network replay", async () => {
  const { props, request } = owner(); const view = render(<FreebuffCatalogPanel {...props} secretGrant={null} />);
  await act(async () => { selectAndRead(); }); expect(props.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(request).not.toHaveBeenCalled();
  view.rerender(<FreebuffCatalogPanel {...props} />); expect(request).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("客户端"), { target: { value: "desktop" } });
  expect(screen.getByRole("button", { name: "检查会话与余额" })).toBeDisabled(); expect(request).not.toHaveBeenCalled();
});
it.each(["revision", "providerId", "credentialId", "businessOrigin", "client", "modelKey"])("rejects response identity mismatch: %s", async (field) => {
  const { props, request, value } = owner(); request.mockResolvedValue({ ...value, [field]: "foreign" });
  render(<FreebuffCatalogPanel {...props} />); await act(async () => { selectAndRead(); });
  expect(screen.getByText(/检查失败/)).toBeInTheDocument(); expect(screen.queryByText("无活动会话")).not.toBeInTheDocument();
});
it.each([
  [{ state: "unavailable" }, "Freebucks 余额暂不可用"],
  [{ state: "available", balance: 5, dailyRemaining: 3, walletBalance: 2, sessionPrice: null }, "模型未提供 Freebucks 定价"],
  [{ state: "available", balance: 5, dailyRemaining: 3, walletBalance: 2, sessionPrice: 0 }, "新会话参考价：0 Freebucks（非锁价或扣款凭据）"],
])("preserves meter semantics without claiming free entitlement: %j", async (meter, label) => {
  const { props, request, value } = owner(); request.mockResolvedValue({ ...value, meter });
  render(<FreebuffCatalogPanel {...props} />); await act(async () => { selectAndRead(); }); expect(screen.getByText(label)).toBeInTheDocument();
});
it("shows active as unclaimed and never sends admission/heartbeat/end", async () => {
  const { props, request, value } = owner(); request.mockResolvedValue({ ...value,
    session: { status: "active", modelKey: "other", admittedAt: value.observedAt, expiresAt: "2026-11-06T01:00:00Z", remainingMs: 500 } });
  render(<FreebuffCatalogPanel {...props} />); await act(async () => { selectAndRead(); });
  expect(screen.getByText("检测到活动会话（未认领）：other")).toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(1);
});
it("rejects wire secrets, malformed amounts and states instead of inventing zeroes", () => {
  const { value } = owner();
  for (const patch of [{ handle: "secret" }, { instanceId: "secret" }, { fetchId: "secret" },
    { session: { status: "queued" } }, { session: { status: "active", modelKey: "open" } },
    { meter: { state: "available", balance: -1, dailyRemaining: 0, walletBalance: 0, sessionPrice: 0 } },
    { meter: { state: "available", balance: 0, dailyRemaining: 0, walletBalance: 0 } }, { observedAt: "bad" }]) {
    expect(freebuffInspectionResponse.safeParse({ ...value, ...patch }).success).toBe(false);
  }
});
it("limits model choices to the exact saved account, not inherited or unrelated models", () => {
  const provider = { preset: "freebuff", supported_models: ["inherited"], credentials: [
    { id: "a", supported_models: ["foreign"] }, { id: "b", supported_models: ["open", "open", "*", 3],
      extra_body: { freebuffBusiness: { origin: "https://codebuff.com", client: "cli" } } },
  ] };
  expect(savedFreebuffModelKeys(provider, "b")).toEqual(["open"]); expect(savedFreebuffModelKeys(provider, "a")).toEqual([]);
});
