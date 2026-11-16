import { screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi, preserveCommittedRouteDocument } from "./BrowserConsoleApp.api-fixture";
import { renderWithProviders, waitForConsoleReady, openWorkspace, flipProviderCard, waitForCommittedRouteDraft } from "./BrowserConsoleApp.render-fixture";

const base = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const override = base.replace("workspace-a", "account-b");
async function setup() {
  window.localStorage.clear();
  const api = createConsoleApi(); const initial = await api.getRouteConfig("management-secret");
  vi.mocked(api.getRouteConfig).mockResolvedValue({ routeConfig: { ...initial.routeConfig, secrets: [
    { path: "/providers/0/credentials/0/api_key", configured: true },
    { path: "/providers/0/credentials/1/api_key", configured: true },
  ], document: { providers: [{ id: "ordinary", label: "Qwen Ordinary", preset: "qwen-official-api", base_url: base,
    explicit_credentials_only: true, credentials: [{ id: "a", supported_models: ["manual"] }, { id: "b", base_url: override }] }],
    model_routes: [], aliases: {} } } });
  await preserveCommittedRouteDocument(api);
  const user = userEvent.setup(); renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
  await waitForConsoleReady(); await openWorkspace(user, /凭据池/i);
  const back = await flipProviderCard(user, /^Qwen Ordinary-api$/);
  await user.click(within(back).getByRole("button", { name: "Qwen 默认目标" }));
  return { api, user };
}
it("edits a normal Qwen parent through the card and autosave, explicitly pinning existing audiences", async () => {
  const { api, user } = await setup();
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
  await user.selectOptions(screen.getByLabelText("Qwen 地域"), "ap-southeast-1");
  await user.clear(screen.getByLabelText("Qwen 工作空间")); await user.type(screen.getByLabelText("Qwen 工作空间"), "workspace-new");
  expect(screen.getByRole("button", { name: "应用到草稿" })).toBeDisabled();
  await user.click(screen.getByRole("checkbox", { name: /固定原目标/ }));
  await user.click(screen.getByRole("button", { name: "应用到草稿" }));
  const document = await waitForCommittedRouteDraft(api);
  expect(document.providers[0]).toMatchObject({ base_url: "https://workspace-new.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1",
    credentials: [{ id: "a", base_url: base, supported_models: ["manual"] }, { id: "b", base_url: override }] });
  const request = vi.mocked(api.commitRouteConfig).mock.calls.at(-1)![1];
  expect(request.expectedRevision).toBe("r1-deadbeefcafe");
  expect(request.secretPatches).toEqual([
    { path: "/providers/0/credentials/0/api_key", operation: "keep" },
    { path: "/providers/0/credentials/1/api_key", operation: "keep" },
  ]);
  await waitFor(() => expect(screen.getByRole("button", { name: "Qwen 默认目标" })).toBeEnabled());
  await user.click(screen.getByRole("button", { name: "Qwen 默认目标" }));
  expect(screen.getByLabelText("Qwen 工作空间")).toHaveValue("workspace-new");
  expect(api.probeCredential).not.toHaveBeenCalled(); expect(api.probeProvider).not.toHaveBeenCalled();
}, 15000);
it("cancel discards an incomplete parent target without committing or probing", async () => {
  const { api, user } = await setup();
  await user.clear(screen.getByLabelText("Qwen 工作空间"));
  await user.click(screen.getByRole("button", { name: "取消" }));
  await user.click(screen.getByRole("button", { name: "Qwen 默认目标" }));
  expect(screen.getByLabelText("Qwen 工作空间")).toHaveValue("workspace-a");
  expect(api.commitRouteConfig).not.toHaveBeenCalled(); expect(api.probeCredential).not.toHaveBeenCalled();
});
it("keeps the staged default visible after a failed autosave, without claiming it is active", async () => {
  const { api, user } = await setup();
  vi.mocked(api.commitRouteConfig).mockRejectedValueOnce(new Error("Qwen fixture save failed"));
  await user.clear(screen.getByLabelText("Qwen 工作空间")); await user.type(screen.getByLabelText("Qwen 工作空间"), "pending-workspace");
  await user.click(screen.getByRole("checkbox", { name: /固定原目标/ }));
  await user.click(screen.getByRole("button", { name: "应用到草稿" }));
  await waitFor(() => expect(screen.getByText("Qwen fixture save failed")).toBeInTheDocument(), { timeout: 4000 });
  expect(api.commitRouteConfig).toHaveBeenCalledTimes(1);
  await user.click(screen.getByRole("button", { name: "Qwen 默认目标" }));
  expect(screen.getByLabelText("Qwen 工作空间")).toHaveValue("pending-workspace");
  expect(screen.getByText(/只有保存成功才生效/)).toBeInTheDocument();
  await user.clear(screen.getByLabelText("Qwen 工作空间")); await user.type(screen.getByLabelText("Qwen 工作空间"), "retry-workspace");
  await user.click(screen.getByRole("button", { name: "应用到草稿" }));
  const document = await waitForCommittedRouteDraft(api, 1);
  expect(document.providers[0]).toMatchObject({ base_url: base.replace("workspace-a", "retry-workspace"),
    credentials: [{ id: "a", base_url: base }, { id: "b", base_url: override }] });
}, 15000);
