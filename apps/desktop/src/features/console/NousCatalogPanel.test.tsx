import { act, fireEvent, render as renderBase, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { expect, it, vi } from "vitest";
import { NousCatalogPanel } from "./NousCatalogPanel";
import type { NousCatalogProps } from "./useNousCatalog";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const render = (ui: Parameters<typeof renderBase>[0]) => renderBase(ui, { wrapper: NeuroProvider });
function owner() {
  const value = { revision: "r1", providerId: "pool", credentialId: "account", catalog: { models: [{ id: "org/model" }] } };
  const request = vi.fn().mockResolvedValue(value); const apply = vi.fn().mockReturnValue(true);
  const props: NousCatalogProps = { client: { request }, expectedRevision: "r1", managementToken: "owner", secretGrant: "grant", disabled: false,
    binding: { providerId: "pool", credentialId: "account", context: "exact-context", baseUrl: "https://inference-api.nousresearch.com/v1" }, onRequestSecretAccess: vi.fn() };
  return { props, request, apply, value };
}
const read = async () => { await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取 Nous 目录" })); }); };
it("only reads on explicit action and stages after a separate apply", async () => {
  const { props, request, apply, value } = owner(); render(<NousCatalogPanel {...props} onApply={apply} />);
  expect(request).not.toHaveBeenCalled(); await read(); expect(apply).not.toHaveBeenCalled();
  expect(request).toHaveBeenCalledExactlyOnceWith("/v1/internal/gateway/console/nous-model-catalog", expect.anything(), expect.objectContaining({
    method: "POST", secretGrant: "grant", body: { providerId: "pool", credentialId: "account", expectedRevision: "r1" },
  }));
  fireEvent.click(screen.getByRole("button", { name: "应用到表单" })); expect(apply).toHaveBeenCalledWith(value.catalog);
  expect(screen.queryByText("org/model")).not.toBeInTheDocument(); expect(request).toHaveBeenCalledOnce();
});
it.each(["revision", "providerId", "credentialId"])("rejects mismatched %s", async (field) => {
  const { props, request, apply, value } = owner(); request.mockResolvedValue({ ...value, [field]: "foreign" });
  render(<NousCatalogPanel {...props} onApply={apply} />); await read();
  expect(screen.getByText("目录读取失败或账号版本已变化。")).toBeInTheDocument(); expect(apply).not.toHaveBeenCalled();
});
it.each([{ expectedRevision: "r2" }, { managementToken: "other" }, { secretGrant: null }, { disabled: true }, { binding: null }])("fences late directory delivery %j", async (patch) => {
  const { props, request, apply, value } = owner(); let resolve!: (v: unknown) => void;
  request.mockImplementation(() => new Promise((done) => { resolve = done; }));
  const view = render(<NousCatalogPanel {...props} onApply={apply} />); await read();
  const signal = request.mock.calls[0][2].signal;
  view.rerender(<NousCatalogPanel {...props} {...patch} onApply={apply} />); expect(signal.aborted).toBe(true);
  await act(async () => { resolve(value); }); expect(screen.queryByText("org/model")).not.toBeInTheDocument();
});
it("requires renewed explicit read after secret consent and hides upstream diagnostics", async () => {
  const { props, request, apply } = owner(); const view = render(<NousCatalogPanel {...props} secretGrant={null} onApply={apply} />);
  await read(); expect(props.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(request).not.toHaveBeenCalled();
  view.rerender(<NousCatalogPanel {...props} onApply={apply} />); expect(request).not.toHaveBeenCalled();
  request.mockRejectedValue(new Error("upstream-secret")); await read();
  expect(screen.getByText("目录读取失败或账号版本已变化。")).toBeInTheDocument(); expect(screen.queryByText(/upstream-secret/)).not.toBeInTheDocument();
});
