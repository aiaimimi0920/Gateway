import { useLayoutEffect, useRef, type ReactNode } from "react";
import { IconButton, NavigationItem } from "@neuro/ui";
import {
  ChevronLeft,
  Languages,
  Minus,
  Monitor,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  RefreshCw,
  Square,
  Sun,
  X,
} from "lucide-react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useUiTheme } from "../../theme/UiThemeProvider";
import { SettingsSearch } from "../settings/SettingsSearch";
import { ShellPageTitle } from "./ShellPageTitle";

export type ShellNavItem<Id extends string = string> = {
  id: Id;
  label: string;
  icon: ReactNode;
};

type WindowCommand = "minimize" | "toggle-maximize" | "close";

async function runWindowCommand(command: WindowCommand): Promise<void> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const currentWindow = getCurrentWindow();
    if (command === "minimize") {
      await currentWindow.minimize();
      return;
    }
    if (command === "toggle-maximize") {
      await currentWindow.toggleMaximize();
      return;
    }
    await currentWindow.close();
  } catch {
    // Window controls only exist inside the Tauri runtime; ignore in browser preview.
  }
}

/**
 * Brand mark: two brackets form the gateway channel, the inner wave is the
 * signal being routed through it. Drawn on a 24-unit grid with stroke-only
 * geometry so it stays legible down to 16px.
 */
export function GatewayMark() {
  return (
    <svg className="nt-mark" viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M9.1 4.8H6.2a1.6 1.6 0 0 0-1.6 1.6v11.2a1.6 1.6 0 0 0 1.6 1.6h2.9M14.9 4.8h2.9a1.6 1.6 0 0 1 1.6 1.6v11.2a1.6 1.6 0 0 1-1.6 1.6h-2.9"
        fill="none"
        stroke="var(--nt-brand-primary)"
        strokeWidth="1.7"
        strokeLinecap="round"
      />
      <path
        d="M8.4 13.4l1.7-3.4 1.9 4.6 1.7-4 1.9 2.8"
        fill="none"
        stroke="var(--nt-brand-secondary)"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function ThemeToggleButton() {
  const { mode, cycleMode } = useUiTheme();
  const { t } = useUiLocale();
  const label =
    mode === "system"
      ? t("主题：跟随系统", "Theme: system")
      : mode === "dark"
        ? t("主题：深色", "Theme: dark")
        : t("主题：浅色", "Theme: light");

  return (
    <IconButton compact variant="quiet"
      className="nt-theme-toggle"
      title={label}
      label={label}
      onClick={cycleMode}
    >
      {mode === "system" ? (
        <Monitor size={16} aria-hidden="true" />
      ) : mode === "dark" ? (
        <Moon size={16} aria-hidden="true" />
      ) : (
        <Sun size={16} aria-hidden="true" />
      )}
    </IconButton>
  );
}

export function LocaleToggleButton() {
  const { locale, toggleLocale, t } = useUiLocale();
  const label = locale === "zh-CN" ? t("切换到英文", "Switch to English") : t("切换到中文", "Switch to Chinese");

  return (
    <IconButton compact variant="quiet"
      className="nt-locale-toggle"
      title={label}
      label={label}
      onClick={toggleLocale}
    >
      <Languages size={16} aria-hidden="true" />
    </IconButton>
  );
}

export type AppShellProps<Id extends string = string> = {
  productName: string;
  navLabel: string;
  navItems: readonly ShellNavItem<Id>[];
  activeNavId: Id;
  onNavigate(id: Id): void;
  utilityItems?: readonly ShellNavItem<Id>[];
  railCollapsed: boolean;
  onToggleRailCollapsed(): void;
  railFooter?: ReactNode;
  title: string;
  headerLeading?: ReactNode;
  headerActions?: ReactNode;
  onBack?: () => void;
  backLabel?: string;
  onRefresh?: () => void;
  refreshBusy?: boolean;
  showWindowControls?: boolean;
  showThemeToggle?: boolean;
  showLocaleToggle?: boolean;
  titlebarActions?: ReactNode;
  beforeStage?: ReactNode;
  shellClassName?: string;
  stageClassName?: string;
  children: ReactNode;
};

export function AppShell<Id extends string = string>({
  productName,
  navLabel,
  navItems,
  activeNavId,
  onNavigate,
  utilityItems,
  railCollapsed,
  onToggleRailCollapsed,
  railFooter,
  title,
  headerLeading,
  headerActions,
  onBack,
  backLabel,
  onRefresh,
  refreshBusy = false,
  showWindowControls = false,
  showThemeToggle = true,
  showLocaleToggle = false,
  titlebarActions,
  beforeStage,
  shellClassName,
  stageClassName,
  children,
}: AppShellProps<Id>) {
  const { t } = useUiLocale();
  const toggleLabel = railCollapsed ? t("展开侧栏", "Expand sidebar") : t("收起侧栏", "Collapse sidebar");
  const settings = activeNavId === "settings";
  const consoleShell = shellClassName?.includes("console") || shellClassName?.includes("settings-native");
  const toggleRef = useRef<HTMLButtonElement>(null);
  const restoreToggleFocus = useRef(false);
  useLayoutEffect(() => {
    // Collapsing moves the trigger between the rail and header; retain keyboard focus.
    if (restoreToggleFocus.current) toggleRef.current?.focus();
    restoreToggleFocus.current = false;
  }, [railCollapsed]);
  const toggle = <IconButton compact variant="quiet" className="nt-console-rail-toggle" title={toggleLabel} label={toggleLabel}
    ref={toggleRef} aria-expanded={!railCollapsed} onClick={event => {
      restoreToggleFocus.current = document.activeElement === event.currentTarget;
      onToggleRailCollapsed();
    }}>
    {railCollapsed ? <PanelLeftOpen size={16} aria-hidden="true" /> : <PanelLeftClose size={16} aria-hidden="true" />}
  </IconButton>;
  const brand = <><span className="nt-brand__mark"><GatewayMark /></span><strong className="nt-brand__name">{productName}</strong></>;

  const renderNavItem = (item: ShellNavItem<Id>) => {
    const active = item.id === activeNavId;
    return (
      <NavigationItem
        key={item.id}
        className="nt-rail__item"
        active={active}
        title={item.label}
        aria-label={item.label}
        onClick={() => onNavigate(item.id)}
      >
        <span className="nt-rail__icon" aria-hidden="true">
          {item.icon}
        </span>
        <span className="nt-rail__label">{item.label}</span>
      </NavigationItem>
    );
  };

  return (
    <main
      className={`nt-shell nt-shell--compact${settings ? " nt-shell--settings" : ""}${railCollapsed ? " nt-shell--rail-collapsed" : ""}${shellClassName ? ` ${shellClassName}` : ""
        }`}
    >
      {!railCollapsed && <aside className="nt-rail">
        <div className="nt-brand" data-tauri-drag-region>
          {brand}
        </div>
        <div className="nt-rail__context">{toggle}</div>

        <nav className="nt-rail__nav" aria-label={navLabel}>
          {navItems.map(renderNavItem)}
        </nav>

        {utilityItems?.length || railFooter ? (
          <div className="nt-rail__utility">
            {railFooter}
            {utilityItems?.length ? (
              <nav className="nt-rail__utility-nav" aria-label={t("辅助导航", "Utility navigation")}>
                {utilityItems.map(renderNavItem)}
              </nav>
            ) : null}
            {settings && headerLeading}
          </div>
        ) : null}
      </aside>}

      <header className="nt-titlebar">
        {/* Window chrome excludes interactive descendants from dragging, including the compact top navigation. */}
        <div className="nt-titlebar__drag" data-tauri-drag-region>
          {railCollapsed && toggle}
          {railCollapsed && <>
            <div className="nt-brand" data-tauri-drag-region>{brand}</div>
            <nav className="nt-top-nav" aria-label={navLabel}>{navItems.map(renderNavItem)}</nav>
          </>}
          <ShellPageTitle title={title} settings={settings} />
        </div>
        <div className="nt-titlebar__controls">
          {(!settings || railCollapsed) && headerLeading}
          {headerActions ? <div className="nt-actions nt-actions--right">{headerActions}</div> : null}
          {onBack ? (
            <IconButton compact variant="quiet"
              className="nt-titlebar__back"
              title={backLabel ?? t("返回", "Back")}
              label={backLabel ?? t("返回", "Back")}
              onClick={onBack}
            >
              <ChevronLeft size={17} aria-hidden="true" />
            </IconButton>
          ) : null}
          {titlebarActions}
          {settings && <SettingsSearch />}
          {railCollapsed && !!utilityItems?.length && <nav className="nt-top-utility" aria-label={t("辅助导航", "Utility navigation")}>{utilityItems.map(renderNavItem)}</nav>}
          {showLocaleToggle && !consoleShell ? <LocaleToggleButton /> : null}
          {showThemeToggle && !consoleShell ? <ThemeToggleButton /> : null}
          {onRefresh ? (
            <IconButton compact variant="quiet"
              className="nt-titlebar__refresh"
              title={t("刷新", "Refresh")}
              label={t("刷新", "Refresh")}
              loading={refreshBusy}
              onClick={onRefresh}
            >
              <RefreshCw size={16} aria-hidden="true" />
            </IconButton>
          ) : null}
          {showWindowControls ? (
            <>
              <IconButton compact variant="quiet"
                className="gateway-window-action"
                title={t("最小化", "Minimize")}
                label={t("最小化", "Minimize")}
                onClick={() => void runWindowCommand("minimize")}
              >
                <Minus size={15} aria-hidden="true" />
              </IconButton>
              <IconButton compact variant="quiet"
                className="gateway-window-action"
                title={t("最大化", "Toggle maximize")}
                label={t("最大化", "Toggle maximize")}
                onClick={() => void runWindowCommand("toggle-maximize")}
              >
                <Square size={13} aria-hidden="true" />
              </IconButton>
              <IconButton compact variant="quiet"
                className="gateway-window-action"
                title={t("关闭", "Close")}
                label={t("关闭", "Close")}
                onClick={() => void runWindowCommand("close")}
              >
                <X size={15} aria-hidden="true" />
              </IconButton>
            </>
          ) : null}
        </div>
      </header>

      <section className="nt-board">
        {beforeStage}

        <section className={`nt-stage${stageClassName ? ` ${stageClassName}` : ""}`}>{children}</section>
      </section>
    </main>
  );
}
