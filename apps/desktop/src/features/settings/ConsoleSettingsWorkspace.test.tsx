import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { UiThemeProvider } from "../../theme/UiThemeProvider";
import { deferred, renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { ConsoleSettingsWorkspace } from "./ConsoleSettingsWorkspace";

const api = vi.hoisted(() => ({ list: vi.fn(), add: vi.fn(), revoke: vi.fn(), edit: vi.fn(), reveal: vi.fn() }));
vi.mock("./managementKeysApi", () => ({ managementKeysApi: () => api }));
const initial = { id: "initial", name: "Initial", createdAt: "", current: true };

function LocaleToggle() {
  const { toggleLocale } = useUiLocale();
  return <button onClick={toggleLocale}>Toggle locale</button>;
}

function setup() {
  renderWithProviders(<UiThemeProvider><LocaleToggle /><ConsoleSettingsWorkspace /></UiThemeProvider>);
  return userEvent.setup();
}

function category(name: RegExp) {
  return within(screen.getByRole("tablist", { name: /设置分类|Settings categories/ })).getByRole("tab", { name });
}

async function openSecurity(user: ReturnType<typeof userEvent.setup>) {
  await user.click(category(/管理安全/));
  await screen.findByText("Initial");
}

describe("Console settings categories", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    localStorage.clear();
    api.list.mockResolvedValue({ keys: [initial] });
    api.add.mockResolvedValue({ success: true });
  });

  it("separates local preferences from gateway security and loads keys only on demand", async () => {
    const user = setup();
    expect(category(/界面偏好/)).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("combobox", { name: "主题" })).toBeVisible();
    expect(screen.queryByText("选择后自动保存")).not.toBeInTheDocument();
    expect(api.list).not.toHaveBeenCalled();
    await openSecurity(user);
    expect(category(/管理安全/)).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("tabpanel")).toHaveClass("nt-settings-workspace__content--security");
    expect(screen.getByRole("heading", { name: "管理密钥" })).toBeVisible();
    expect(screen.queryByRole("radio")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^管理密钥$/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "删除 Initial" })).toBeDisabled();
    await user.click(category(/界面偏好/));
    expect(screen.getByRole("tabpanel")).not.toHaveClass("nt-settings-workspace__content--security");
    expect(screen.queryByRole("list", { name: "管理密钥列表" })).not.toBeInTheDocument();
    expect(api.add).not.toHaveBeenCalled();
    expect(api.edit).not.toHaveBeenCalled();
    expect(api.revoke).not.toHaveBeenCalled();
  });

  it("uses compact theme and language selects with immediate local persistence", async () => {
    const user = setup();
    await user.selectOptions(screen.getByRole("combobox", { name: "主题" }), "light");
    expect(screen.getByRole("combobox", { name: "主题" })).toHaveValue("light");
    expect(localStorage.getItem("gateway-ui-theme")).toBe("light");
    expect(document.documentElement.dataset.ntTheme).toBe("light");
    await user.selectOptions(screen.getByRole("combobox", { name: "界面语言" }), "en-US");
    expect(screen.getByRole("combobox", { name: "Language" })).toHaveValue("en-US");
    expect(localStorage.getItem("gateway-ui-locale")).toBe("en-US");
    expect(api.list).not.toHaveBeenCalled();
  });

  it("protects an unsaved key draft from category and locale changes", async () => {
    const user = setup();
    await openSecurity(user);
    await user.click(screen.getByRole("button", { name: "添加" }));
    await user.type(screen.getByLabelText("密钥名称"), "Draft key");
    await user.type(screen.getByLabelText("管理密钥", { exact: true }), "test-only-draft");
    expect(category(/界面偏好/)).toBeDisabled();
    expect(category(/诊断与日志/)).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Toggle locale" }));
    expect(screen.getByLabelText("Key name")).toHaveValue("Draft key");
    expect(screen.getByLabelText("Management key", { exact: true })).toHaveValue("test-only-draft");
    expect(api.list).toHaveBeenCalledOnce();
    expect(JSON.stringify(localStorage)).not.toContain("test-only-draft");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(category(/Appearance/)).toBeEnabled();
    expect(screen.getByRole("button", { name: "Add" })).toHaveFocus();
    expect(api.add).not.toHaveBeenCalled();
  });

  it("does not detach or duplicate an in-flight mutation when the locale changes", async () => {
    const pending = deferred<{ success: true }>();
    api.add.mockReturnValue(pending.promise);
    const user = setup();
    await openSecurity(user);
    await user.click(screen.getByRole("button", { name: "添加" }));
    await user.type(screen.getByLabelText("密钥名称"), "Second");
    await user.type(screen.getByLabelText("管理密钥", { exact: true }), "test-only-key");
    await user.click(screen.getByRole("button", { name: "保存" }));
    await user.click(screen.getByRole("button", { name: "Toggle locale" }));
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    expect(category(/Appearance/)).toBeDisabled();
    expect(api.list).toHaveBeenCalledOnce();
    api.list.mockResolvedValue({ keys: [initial, { ...initial, id: "second", name: "Second", current: false }] });
    await act(async () => pending.resolve({ success: true }));
    await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
    expect(screen.getByText("Second")).toBeVisible();
    expect(category(/Appearance/)).toBeEnabled();
    expect(api.add).toHaveBeenCalledExactlyOnceWith("management-secret", "Second", "test-only-key");
    expect(api.list).toHaveBeenCalledTimes(2);
  });

  it("keeps a load failure visible and translates it without reloading or leaking raw errors", async () => {
    api.list.mockRejectedValue(new Error("private-server-detail"));
    const user = setup();
    await user.click(category(/管理安全/));
    expect(await screen.findByRole("alert")).toHaveTextContent("无法加载管理密钥");
    await user.click(screen.getByRole("button", { name: "Toggle locale" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Could not load management keys");
    expect(document.body).not.toHaveTextContent("private-server-detail");
    expect(api.list).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "Add" })).toBeDisabled();
  });
});
