import { act, fireEvent, render as renderBase, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { expect, it, vi } from "vitest";
import { DimCatalogPanel } from "./DimCatalogPanel";
import type { DimCatalogProps } from "./useDimCatalog";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const render = (ui: Parameters<typeof renderBase>[0]) => renderBase(ui, { wrapper: NeuroProvider });
function owner() {
  const catalog = { site: "china", client_id: "allowed-client", subject: "account-subject", models: [{ id: "opaque-model", name: "测试模型", protocol: "messages",
    context_window: null, max_output_tokens: null, tools: true, reasoning: null, reasoning_efforts: [], input_modalities: ["text"], output_modalities: ["text"],
    service_tiers: [], deprecated: false, zdr: false }] };
  const value = { revision: "r1", providerId: "pool", credentialId: "account", catalog };
  const request = vi.fn().mockResolvedValue(value); const apply = vi.fn().mockReturnValue(false);
  const props: DimCatalogProps = { client: { request }, expectedRevision: "r1", managementToken: "operator", secretGrant: "grant", disabled: false,
    binding: { providerId: "pool", credentialId: "account", site: "china", clientId: "allowed-client", subject: "account-subject", baseUrl: "https://dimagent.cn/v1" }, onRequestSecretAccess: vi.fn() };
  return { props, request, apply, value };
}
it("does not fetch on mount; reads only the exact saved account and stages only after explicit application", async () => {
  const { props, request, apply, value } = owner(); render(<DimCatalogPanel {...props} onApply={apply} />);
  expect(request).not.toHaveBeenCalled(); expect(apply).not.toHaveBeenCalled();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取 DIM 目录" })); });
  expect(request).toHaveBeenCalledExactlyOnceWith("/v1/internal/gateway/console/dim-model-catalog", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "operator", secretGrant: "grant", signal: expect.any(AbortSignal), body: { providerId: "pool", credentialId: "account", expectedRevision: "r1" },
  }));
  expect(screen.getByText("测试模型")).toBeInTheDocument(); expect(apply).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "应用 DIM 目录到表单" })); expect(apply).toHaveBeenCalledWith(value.catalog);
  expect(screen.getByText("测试模型")).toBeInTheDocument(); apply.mockReturnValue(true);
  fireEvent.click(screen.getByRole("button", { name: "应用 DIM 目录到表单" })); expect(screen.queryByText("测试模型")).not.toBeInTheDocument();
  expect(request).toHaveBeenCalledOnce();
});
it.each(["revision", "providerId", "credentialId"])("rejects a different response %s", async (field) => {
  const { props, request, apply, value } = owner(); request.mockResolvedValue({ ...value, [field]: "foreign" });
  render(<DimCatalogPanel {...props} onApply={apply} />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取 DIM 目录" })); });
  expect(screen.getByText(/目录响应与当前 DIM/)).toBeInTheDocument(); expect(apply).not.toHaveBeenCalled();
});
it.each(["site", "client_id", "subject"])("rejects a different directory %s", async (field) => {
  const { props, request, apply, value } = owner(); request.mockResolvedValue({ ...value, catalog: { ...value.catalog, [field]: "foreign" } });
  render(<DimCatalogPanel {...props} onApply={apply} />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取 DIM 目录" })); });
  expect(screen.getByText(/目录响应与当前 DIM/)).toBeInTheDocument(); expect(apply).not.toHaveBeenCalled();
});
it.each([{ expectedRevision: "r2" }, { managementToken: "other" }, { secretGrant: null }, { disabled: true }, { binding: null }, { client: { request: vi.fn() } }])(
  "aborts and rejects late results when read ownership changes: %j", async (patch) => {
    const { props, request, apply, value } = owner(); let resolve!: (value: unknown) => void;
    request.mockImplementation(() => new Promise((done) => { resolve = done; }));
    const view = render(<DimCatalogPanel {...props} onApply={apply} />);
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取 DIM 目录" })); });
    const signal = request.mock.calls[0][2].signal; view.rerender(<DimCatalogPanel {...props} {...patch} onApply={apply} />);
    expect(signal.aborted).toBe(true); await act(async () => { resolve(value); });
    expect(screen.queryByText("测试模型")).not.toBeInTheDocument(); expect(apply).not.toHaveBeenCalled();
  });
it("aborts read ownership on cancellation and unmount", async () => {
  const { props, request, apply, value } = owner(); let resolve!: (value: unknown) => void;
  request.mockImplementation(() => new Promise((done) => { resolve = done; }));
  const view = render(<DimCatalogPanel {...props} onApply={apply} />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取 DIM 目录" })); });
  const signal = request.mock.calls[0][2].signal; fireEvent.click(screen.getByRole("button", { name: "取消读取" })); expect(signal.aborted).toBe(true);
  await act(async () => { resolve(value); }); expect(screen.queryByText("测试模型")).not.toBeInTheDocument();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取 DIM 目录" })); });
  view.unmount(); expect(request.mock.calls[1][2].signal.aborted).toBe(true);
});
it("restores consent only after an explicit new read and hides upstream diagnostics", async () => {
  const { props, request, apply } = owner(); const view = render(<DimCatalogPanel {...props} secretGrant={null} onApply={apply} />);
  fireEvent.click(screen.getByRole("button", { name: "读取 DIM 目录" })); expect(props.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(request).not.toHaveBeenCalled();
  view.rerender(<DimCatalogPanel {...props} onApply={apply} />); expect(request).not.toHaveBeenCalled();
  request.mockRejectedValue(new Error("upstream-secret"));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取 DIM 目录" })); });
  expect(screen.getByText(/DIM 目录读取失败/)).toBeInTheDocument(); expect(screen.queryByText(/upstream-secret/)).not.toBeInTheDocument();
});
