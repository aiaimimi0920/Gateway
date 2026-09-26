import type { ReactNode } from "react";
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
    <button
      className="nt-shell-icon-button nt-theme-toggle"
      type="button"
      title={label}
      aria-label={label}
      onClick={cycleMode}
    >
      {mode === "system" ? (
        <Monitor size={16} aria-hidden="true" />
      ) : mode === "dark" ? (
        <Moon size={16} aria-hidden="true" />
      ) : (
        <Sun size={16} aria-hidden="true" />
      )}
    </button>
  );
}

export function LocaleToggleButton() {
  const { locale, toggleLocale, t } = useUiLocale();
  const label = locale === "zh-CN" ? t("切换到英文", "Switch to English") : t("切换到中文", "Switch to Chinese");

  return (
    <button
      className="nt-shell-icon-button nt-locale-toggle"
      type="button"
      title={label}
      aria-label={label}
      onClick={toggleLocale}
    >
      <Languages size={16} aria-hidden="true" />
    </button>
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

  const renderNavItem = (item: ShellNavItem<Id>) => {
    const active = item.id === activeNavId;
    return (
      <button
        key={item.id}
        className={`nt-rail__item${active ? " nt-rail__item--active" : ""}`}
        type="button"
        title={item.label}
        aria-label={item.label}
        aria-current={active ? "page" : undefined}
        onClick={() => onNavigate(item.id)}
      >
        <span className="nt-rail__icon" aria-hidden="true">
          {item.icon}
        </span>
        <span className="nt-rail__label">{item.label}</span>
      </button>
    );
  };

  return (
    <main
      className={`nt-shell${railCollapsed ? " nt-shell--rail-collapsed" : ""}${
        shellClassName ? ` ${shellClassName}` : ""
      }`}
    >
      <aside className="nt-rail">
        <div className="nt-brand">
          <button
            className="nt-console-rail-toggle"
            type="button"
            title={toggleLabel}
            aria-label={toggleLabel}
            aria-expanded={!railCollapsed}
            onClick={onToggleRailCollapsed}
          >
            {railCollapsed ? (
              <PanelLeftOpen size={17} aria-hidden="true" />
            ) : (
              <PanelLeftClose size={17} aria-hidden="true" />
            )}
          </button>
          <span className="nt-brand__mark">
            <GatewayMark />
          </span>
          <strong className="nt-brand__name">{productName}</strong>
        </div>

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
          </div>
        ) : null}
      </aside>

      <header className="nt-titlebar">
        {/* The drag region stays text-free: the workspace title already lives in the board header below. */}
        <div className="nt-titlebar__drag" data-tauri-drag-region>
          {onBack ? (
            <button
              className="nt-shell-icon-button nt-titlebar__back"
              type="button"
              title={backLabel ?? t("返回", "Back")}
              aria-label={backLabel ?? t("返回", "Back")}
              onClick={onBack}
            >
              <ChevronLeft size={17} aria-hidden="true" />
            </button>
          ) : null}
        </div>
        <div className="nt-titlebar__controls">
          {titlebarActions}
          {showLocaleToggle ? <LocaleToggleButton /> : null}
          {showThemeToggle ? <ThemeToggleButton /> : null}
          {onRefresh ? (
            <button
              className="nt-shell-icon-button nt-titlebar__refresh"
              type="button"
              title={t("刷新", "Refresh")}
              aria-label={t("刷新", "Refresh")}
              disabled={refreshBusy}
              onClick={onRefresh}
            >
              <RefreshCw size={16} aria-hidden="true" />
            </button>
          ) : null}
          {showWindowControls ? (
            <>
              <button
                className="nt-window-control"
                type="button"
                title={t("最小化", "Minimize")}
                aria-label={t("最小化", "Minimize")}
                onClick={() => void runWindowCommand("minimize")}
              >
                <Minus size={15} aria-hidden="true" />
              </button>
              <button
                className="nt-window-control"
                type="button"
                title={t("最大化", "Toggle maximize")}
                aria-label={t("最大化", "Toggle maximize")}
                onClick={() => void runWindowCommand("toggle-maximize")}
              >
                <Square size={13} aria-hidden="true" />
              </button>
              <button
                className="nt-window-control nt-window-control--close"
                type="button"
                title={t("关闭", "Close")}
                aria-label={t("关闭", "Close")}
                onClick={() => void runWindowCommand("close")}
              >
                <X size={15} aria-hidden="true" />
              </button>
            </>
          ) : null}
        </div>
      </header>

      <section className="nt-board">
        <header className="nt-board__header">
          <div className="nt-board__heading">
            <h1>{title}</h1>
          </div>
          {headerActions ? <div className="nt-actions nt-actions--right">{headerActions}</div> : null}
        </header>

        {beforeStage}

        <section className={`nt-stage${stageClassName ? ` ${stageClassName}` : ""}`}>{children}</section>
      </section>
    </main>
  );
}
