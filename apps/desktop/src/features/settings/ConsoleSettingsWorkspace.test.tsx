import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { UiThemeProvider } from "../../theme/UiThemeProvider";
import { deferred, renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { ConsoleSettingsWorkspace } from "./ConsoleSettingsWorkspace";

const api = vi.hoisted(() => ({ list: vi.fn(), setSlot: vi.fn() }));
vi.mock("./managementKeysApi", () => ({ managementKeysApi: () => api }));
vi.mock("./gatewayLanSharingApi", () => ({ gatewayLanSharingApi: () => ({ load: async () => ({ enabled: true }) }) }));
vi.mock("./gatewayProxyApi", () => ({ gatewayProxyApi: () => ({ load: async () => ({ mode: "auto", url: "", effectiveMode: "auto", restartRequired: false }) }) }));
const initial = { id: "initial", name: "Initial", createdAt: "", current: true, enabled: true };
function LocaleToggle() { const { toggleLocale } = useUiLocale(); return <button onClick={toggleLocale}>Toggle locale</button>; }
function setup() {
  renderWithProviders(<UiThemeProvider><LocaleToggle /><ConsoleSettingsWorkspace /></UiThemeProvider>);
  return userEvent.setup();
}
function category(name: RegExp) { return screen.getByRole("tab", { name }); }
async function openNetwork(user: ReturnType<typeof userEvent.setup>) {
  await user.click(category(/网络与共享/));
  await waitFor(() => expect(screen.getByLabelText("管理密钥二", { exact: true })).toBeEnabled());
}
beforeEach(() => { vi.resetAllMocks(); localStorage.clear(); api.list.mockResolvedValue({ keys: [initial], slotsSupported: true }); api.setSlot.mockResolvedValue({ success: true }); });

it("loads two inline key slots only when entering network settings", async () => {
  const user = setup();
  expect(category(/常规/)).toHaveAttribute("aria-current", "page");
  expect(api.list).not.toHaveBeenCalled();
  await openNetwork(user);
  expect(category(/网络与共享/)).toHaveAttribute("aria-current", "page");
  expect(screen.getByRole("tabpanel")).toHaveClass("nt-settings-workspace__content--network");
  expect(screen.queryByRole("heading", { name: "面板管理密钥" })).not.toBeInTheDocument();
  expect(document.querySelectorAll(".nt-management-key-slots .nt-setting-row")).toHaveLength(2);
  for (const name of ["不带供应商的模型 id", "允许调用的网页", "繁忙的模型靠后", "在浏览器中访问"]) expect(screen.queryByText(name)).not.toBeInTheDocument();
  expect(screen.getByRole("radiogroup", { name: "局域网共享" })).toBeVisible();
  await user.click(category(/常规/));
  expect(screen.queryByLabelText("管理密钥一", { exact: true })).not.toBeInTheDocument();
  expect(api.setSlot).not.toHaveBeenCalled();
});

it("keeps local appearance and language preferences independent", async () => {
  const user = setup();
  await user.click(screen.getByRole("radio", { name: "浅色" }));
  expect(localStorage.getItem("gateway-ui-theme")).toBe("light");
  await user.selectOptions(screen.getByRole("combobox", { name: "语言" }), "en-US");
  expect(localStorage.getItem("gateway-ui-locale")).toBe("en-US");
  expect(api.list).not.toHaveBeenCalled();
});

it("protects inline drafts from category changes and keeps them on locale changes", async () => {
  const user = setup(); await openNetwork(user);
  await user.type(screen.getByLabelText("管理密钥二", { exact: true }), "draft-secret");
  expect(category(/常规/)).toBeDisabled();
  fireEvent.click(screen.getByText("Toggle locale"));
  expect(screen.getByLabelText("Management key 2", { exact: true })).toHaveValue("draft-secret");
  expect(api.list).toHaveBeenCalledOnce();
  expect(JSON.stringify(localStorage)).not.toContain("draft-secret");
  fireEvent.keyDown(screen.getByLabelText("Management key 2", { exact: true }), { key: "Escape" });
  await waitFor(() => expect(category(/General/)).toBeEnabled());
});

it("does not detach an in-flight slot save when locale changes", async () => {
  const pending = deferred<unknown>(); api.setSlot.mockReturnValue(pending.promise);
  const user = setup(); await openNetwork(user);
  await user.type(screen.getByLabelText("管理密钥二", { exact: true }), "second-secret{Enter}");
  fireEvent.click(screen.getByText("Toggle locale"));
  expect(screen.getByLabelText("Management key 2", { exact: true })).toBeDisabled();
  expect(category(/General/)).toBeDisabled();
  await act(async () => pending.resolve({ success: true }));
  await waitFor(() => expect(category(/General/)).toBeEnabled());
  expect(api.setSlot).toHaveBeenCalledExactlyOnceWith("management-secret", 1, false, "second-secret");
  expect(api.list).toHaveBeenCalledTimes(2);
});

it("keeps a load error localized without leaking server details or retrying on locale changes", async () => {
  api.list.mockRejectedValue(new Error("private-server-detail"));
  const user = setup(); await user.click(category(/网络与共享/));
  expect(await screen.findByRole("alert")).toHaveTextContent("无法加载管理密钥");
  fireEvent.click(screen.getByText("Toggle locale"));
  expect(screen.getByRole("alert")).toHaveTextContent("Could not load management keys");
  expect(document.body).not.toHaveTextContent("private-server-detail");
  expect(api.list).toHaveBeenCalledOnce();
});
