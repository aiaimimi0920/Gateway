import { useState, type ReactNode } from "react";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { UiThemeProvider } from "../../theme/UiThemeProvider";
import { AppShell } from "./AppShell";
import { SettingsHeaderSlot, SettingsNavigationProvider } from "../settings/SettingsNavigation";

const windowApi = vi.hoisted(() => ({ minimize: vi.fn(), toggleMaximize: vi.fn(), close: vi.fn() }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => windowApi }));
const items = [{ id: "accounts", label: "凭据池", icon: <span>●</span> }, { id: "models", label: "模型池", icon: <span>□</span> }];
function Providers({ children }: { children: ReactNode }) {
  return <UiThemeProvider><UiLocaleProvider>{children}</UiLocaleProvider></UiThemeProvider>;
}
function Harness({ refresh = vi.fn(), busy = false, back = vi.fn() }) {
  const [active, setActive] = useState("accounts"), [collapsed, setCollapsed] = useState(false);
  return <AppShell productName="Gateway" navLabel="测试导航" navItems={items} activeNavId={active} onNavigate={setActive}
    railCollapsed={collapsed} onToggleRailCollapsed={() => setCollapsed(value => !value)} title={items.find(item => item.id === active)!.label}
    onRefresh={refresh} refreshBusy={busy} onBack={back} showLocaleToggle showWindowControls>
    <input aria-label="业务草稿" defaultValue="保留草稿" />
  </AppShell>;
}
describe("standard shell controls", () => {
  beforeEach(() => { window.localStorage.clear(); vi.clearAllMocks(); });

  it("uses standard navigation without rebuilding the selected button or draft", async () => {
    const user = userEvent.setup();
    render(<Providers><Harness /></Providers>);
    const models = within(screen.getByRole("navigation", { name: "测试导航" })).getByRole("button", { name: "模型池" });
    const input = screen.getByLabelText("业务草稿");
    await user.type(input, " 不重置");
    models.focus(); await user.keyboard("{Enter}");
    expect(models).toHaveClass("n-navigation-item");
    expect(models).toHaveAttribute("aria-current", "page");
    expect(models).toHaveFocus();
    expect(screen.getByRole("button", { name: "模型池" })).toBe(models);
    expect(input).toHaveValue("保留草稿 不重置");
    expect(screen.getByRole("button", { name: "凭据池" })).not.toHaveAttribute("aria-current");
    expect(screen.getByRole("heading", { name: "模型池" })).toBeInTheDocument();
    expect(document.querySelector(".nt-board__header")).toBeNull();
  });

  it("keeps the settings breadcrumb and tools in one header and follows category changes", async () => {
    const user = userEvent.setup();
    render(<Providers><SettingsNavigationProvider>
      <AppShell productName="Gateway" navLabel="测试导航" navItems={items} activeNavId="settings" onNavigate={vi.fn()}
        utilityItems={[{ id: "settings", label: "设置", icon: null }]} railCollapsed={false} onToggleRailCollapsed={vi.fn()}
        title="设置" headerLeading={<SettingsHeaderSlot />} onRefresh={vi.fn()}>
        <div>设置内容</div>
      </AppShell>
    </SettingsNavigationProvider></Providers>);
    const title = screen.getByRole("heading", { name: /设置\s*常规/ });
    expect(title.closest("header")).toBe(screen.getByRole("combobox", { name: "查找设置" }).closest("header"));
    expect(title.closest("header")).toBe(screen.getByRole("button", { name: "刷新" }).closest("header"));
    expect(screen.queryByRole("tab", { name: "隐私" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "同步与备份" }));
    expect(screen.getByRole("heading", { name: /设置\s*同步与备份/ })).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "关于" }));
    expect(screen.getByRole("heading", { name: /设置\s*关于/ })).toBeInTheDocument();
    expect(document.querySelector(".nt-board__header")).toBeNull();
  });

  it("restores focus to the moved collapse trigger in both directions", async () => {
    const user = userEvent.setup();
    render(<Providers><Harness /></Providers>);
    await user.click(screen.getByRole("button", { name: "收起侧栏" }));
    expect(screen.getByRole("navigation", { name: "测试导航" })).toHaveClass("nt-top-nav");
    const expand = screen.getByRole("button", { name: "展开侧栏" });
    expect(expand).toHaveFocus(); expect(expand).toHaveAttribute("aria-expanded", "false");
    await user.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "收起侧栏" })).toHaveFocus();
    expect(document.querySelector(".nt-rail")).not.toBeNull();
  });

  it("keeps refresh identity and label while busy, and preserves callbacks", async () => {
    const user = userEvent.setup(), refresh = vi.fn(), back = vi.fn();
    const view = render(<Providers><Harness refresh={refresh} back={back} /></Providers>);
    const action = screen.getByRole("button", { name: "刷新" });
    await user.click(action); await user.click(screen.getByRole("button", { name: "返回" }));
    expect(refresh).toHaveBeenCalledOnce(); expect(back).toHaveBeenCalledOnce();
    view.rerender(<Providers><Harness refresh={refresh} back={back} busy /></Providers>);
    expect(screen.getByRole("button", { name: "刷新" })).toBe(action);
    expect(action).toHaveAttribute("aria-busy", "true"); expect(action).toBeDisabled();
    await user.click(action); expect(refresh).toHaveBeenCalledOnce();
  });

  it("keeps theme and locale ownership in their providers", async () => {
    const user = userEvent.setup();
    render(<Providers><Harness /></Providers>);
    await user.click(screen.getByRole("button", { name: "主题：深色" }));
    expect(document.documentElement).toHaveAttribute("data-nt-theme", "light");
    await user.click(screen.getByRole("button", { name: "切换到英文" }));
    expect(screen.getByRole("button", { name: "Theme: light" })).toHaveClass("n-icon-button");
    expect(screen.getByRole("button", { name: "Refresh" })).toBeInTheDocument();
  });

  it("preserves native window command routing", async () => {
    const user = userEvent.setup();
    render(<Providers><Harness /></Providers>);
    for (const [label, callback] of [["最小化", windowApi.minimize], ["最大化", windowApi.toggleMaximize], ["关闭", windowApi.close]] as const) {
      await user.click(screen.getByRole("button", { name: label }));
      expect(callback).toHaveBeenCalledOnce();
    }
  });
});
