import { screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi, preserveCommittedRouteDocument } from "./BrowserConsoleApp.api-fixture";
import { renderWithProviders, waitForConsoleReady, openWorkspace, openProviderSettings, waitForCommittedRouteDraft } from "./BrowserConsoleApp.render-fixture";

async function setup(preset?: string) {
  window.localStorage.clear();
  const api = createConsoleApi(); const initial = await api.getRouteConfig("management-secret");
  vi.mocked(api.getRouteConfig).mockResolvedValue({ routeConfig: { ...initial.routeConfig, secrets: [
    { path: "/providers/0/credentials/0/api_key", configured: true },
  ], document: { providers: [{ id: "stable-id", label: "Original", adapter: "openai_compatible", preset,
    base_url: "https://old.example/v1", explicit_credentials_only: true,
    credentials: [{ id: "account", supported_models: ["text-model"] }],
  }], model_routes: [{ pattern: "text-model", provider_ids: ["stable-id"] }], aliases: {} } } });
  await preserveCommittedRouteDocument(api);
  const user = userEvent.setup(); renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
  await waitForConsoleReady(); await openWorkspace(user, /凭据池/i);
  const dialog = await openProviderSettings(user, /Original/);
  return { api, user, form: within(within(dialog).getByRole("form", { name: "服务商信息" })) };
}

it.each([undefined, "openai"])("edits API/custom provider metadata through the real settings and autosave flow (%s)", async preset => {
  const { api, user, form } = await setup(preset);
  await user.clear(form.getByLabelText("服务商名称")); await user.type(form.getByLabelText("服务商名称"), "My relay");
  await user.clear(form.getByLabelText("目标网址")); await user.type(form.getByLabelText("目标网址"), "https://new.example/v1");
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
  await user.click(form.getByRole("button", { name: "保存" }));
  const document = await waitForCommittedRouteDraft(api);
  expect(document.providers[0]).toMatchObject({ id: "stable-id", label: "My relay", base_url: "https://new.example/v1", credentials: [{ id: "account" }] });
  expect(document.model_routes).toEqual([{ pattern: "text-model", provider_ids: ["stable-id"] }]);
  expect(vi.mocked(api.commitRouteConfig).mock.calls.at(-1)![1].secretPatches).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "keep" }]);
  await waitFor(() => expect(form.getByRole("status")).toHaveTextContent("已保存"));
  await user.click(screen.getByRole("button", { name: "关闭" }));
  const dialog = await openProviderSettings(user, /My relay/);
  expect(within(dialog).getByLabelText("目标网址")).toHaveValue("https://new.example/v1");
  expect(api.probeCredential).not.toHaveBeenCalled();
}, 15000);

it("closing or canceling changes does not save, and malformed targets show an error", async () => {
  const { api, user, form } = await setup();
  await user.clear(form.getByLabelText("目标网址")); await user.type(form.getByLabelText("目标网址"), "invalid-target");
  await user.click(form.getByRole("button", { name: "保存" }));
  expect(form.getByText(/有效的 HTTP/)).toBeInTheDocument();
  await user.click(form.getByRole("button", { name: "取消修改" }));
  expect(form.getByLabelText("目标网址")).toHaveValue("https://old.example/v1");
  await user.clear(form.getByLabelText("服务商名称")); await user.type(form.getByLabelText("服务商名称"), "Unsaved");
  await user.click(screen.getByRole("button", { name: "关闭" }));
  const dialog = await openProviderSettings(user, /Original/);
  expect(within(dialog).getByLabelText("服务商名称")).toHaveValue("Original");
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
});

it("keeps failed saves visible and retries through the existing revision/secret flow", async () => {
  const { api, user, form } = await setup();
  vi.mocked(api.commitRouteConfig).mockRejectedValueOnce(new Error("fixture save failed"));
  await user.clear(form.getByLabelText("服务商名称")); await user.type(form.getByLabelText("服务商名称"), "Retry name");
  await user.click(form.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(form.getByText("fixture save failed")).toBeInTheDocument(), { timeout: 4000 });
  expect(form.getByRole("status")).toHaveTextContent("保存失败");
  expect(api.commitRouteConfig).toHaveBeenCalledTimes(1);
  await user.click(form.getByRole("button", { name: "重试保存" }));
  await waitForCommittedRouteDraft(api, 1);
  await waitFor(() => expect(form.getByRole("status")).toHaveTextContent("已保存"));
}, 15000);
