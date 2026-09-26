import { useMemo, useState } from "react";
import {
  Activity,
  Boxes,
  FlaskConical,
  Rocket,
  ScrollText,
  Settings,
  SlidersHorizontal,
} from "lucide-react";
import { ApiTestPanel } from "./features/api-test/ApiTestPanel";
import { ConfigPanel } from "./features/config/ConfigPanel";
import { LauncherPanel } from "./features/launcher/LauncherPanel";
import { LogsPanel } from "./features/logs/LogsPanel";
import { ModelsPanel } from "./features/models/ModelsPanel";
import { SettingsPanel } from "./features/settings/SettingsPanel";
import { AppShell, type ShellNavItem } from "./features/shell/AppShell";
import { StatusPanel } from "./features/status/StatusPanel";
import type { GatewayDesktopState, SectionId } from "./lib/types";
import { useGatewayDesktopState } from "./state/useGatewayDesktopState";

type LauncherSectionId = SectionId | "settings";

const navigationItems: ShellNavItem<LauncherSectionId>[] = [
  { id: "launcher", label: "启动台", icon: <Rocket size={16} /> },
  { id: "config", label: "运行配置", icon: <SlidersHorizontal size={16} /> },
  { id: "status", label: "运行状态", icon: <Activity size={16} /> },
  { id: "models", label: "模型目录", icon: <Boxes size={16} /> },
  { id: "api-test", label: "接口测试", icon: <FlaskConical size={16} /> },
  { id: "logs", label: "诊断日志", icon: <ScrollText size={16} /> },
];

const utilityItems: ShellNavItem<LauncherSectionId>[] = [
  { id: "settings", label: "设置", icon: <Settings size={16} /> },
];

function renderPanel(activeSection: LauncherSectionId, state: GatewayDesktopState) {
  switch (activeSection) {
    case "launcher":
      return <LauncherPanel state={state} />;
    case "config":
      return <ConfigPanel state={state} />;
    case "status":
      return <StatusPanel state={state} />;
    case "models":
      return <ModelsPanel state={state} />;
    case "api-test":
      return <ApiTestPanel state={state} />;
    case "logs":
      return <LogsPanel state={state} />;
    case "settings":
      return <SettingsPanel state={state} />;
  }
}

function App() {
  const [activeSection, setActiveSection] = useState<LauncherSectionId>("launcher");
  const [railCollapsed, setRailCollapsed] = useState(false);
  const gatewayState = useGatewayDesktopState();

  const activeItem = useMemo(
    () =>
      [...navigationItems, ...utilityItems].find((item) => item.id === activeSection) ??
      navigationItems[0],
    [activeSection],
  );

  const running = gatewayState.processSnapshot.running;
  const browserPreviewMode = !gatewayState.isTauriAvailable;
  const readinessDegraded = Boolean(gatewayState.readyProbe?.ok && gatewayState.readyProbe?.data?.degraded);

  return (
    <AppShell<LauncherSectionId>
      productName="Gateway"
      navLabel="Gateway desktop sections"
      navItems={navigationItems}
      utilityItems={utilityItems}
      activeNavId={activeSection}
      onNavigate={setActiveSection}
      railCollapsed={railCollapsed}
      onToggleRailCollapsed={() => setRailCollapsed((current) => !current)}
      railFooter={
        <div className="nt-rail__status">
          <div className={running ? "nt-chip nt-chip--online" : "nt-chip"}>
            {running ? "RUNNING" : "STANDBY"}
          </div>
          <dl className="nt-mini-list">
            <div>
              <dt>Profile</dt>
              <dd>{gatewayState.draftProfile.name}</dd>
            </div>
            <div>
              <dt>Base URL</dt>
              <dd>{gatewayState.baseUrl}</dd>
            </div>
            <div>
              <dt>Runtime</dt>
              <dd>{gatewayState.isTauriAvailable ? "Tauri" : "Browser preview"}</dd>
            </div>
          </dl>
        </div>
      }
      title={activeItem.label}
      onRefresh={() => void gatewayState.refreshAll()}
      refreshBusy={gatewayState.busy}
      showWindowControls={gatewayState.isTauriAvailable}
      showLocaleToggle
      headerActions={
        <>
          <button
            className={running ? "nt-btn nt-btn--outline" : "nt-btn nt-btn--primary"}
            type="button"
            disabled={
              gatewayState.busy ||
              !gatewayState.isTauriAvailable ||
              (!running && !gatewayState.canStartGateway)
            }
            onClick={() => void (running ? gatewayState.stopGateway() : gatewayState.startGateway())}
          >
            {running ? "停止 Gateway" : "启动 Gateway"}
          </button>
          {gatewayState.startWillSaveDraftProfile ? (
            <span className="nt-start-save-hint">启动会先保存当前草稿</span>
          ) : null}
        </>
      }
      beforeStage={
        <>
          {browserPreviewMode ? (
            <div className="nt-alert nt-alert--warning">
              <span>
                当前处于浏览器预览模式：可以查看界面、测试 HTTP 面板，但本地 sidecar 启停、profile
                持久化和日志读取需要在 Tauri 桌面运行时中使用。
              </span>
            </div>
          ) : null}

          <section className="nt-hud-strip" aria-label="Gateway desktop summary">
            <article className="nt-card nt-card--stat">
              <span>Runtime</span>
              <strong>{gatewayState.runtimeInfo?.gatewayMode ?? "headless-first"}</strong>
            </article>
            <article className="nt-card nt-card--stat">
              <span>Health</span>
              <strong>{gatewayState.healthProbe?.ok ? "OK" : "CHECK"}</strong>
            </article>
            <article className="nt-card nt-card--stat">
              <span>Ready</span>
              <strong>
                {readinessDegraded ? "DEGRADED" : gatewayState.readyProbe?.ok ? "READY" : "PENDING"}
              </strong>
            </article>
          </section>

          {gatewayState.notice ? (
            <div className={`nt-alert nt-alert--${gatewayState.notice.tone}`}>
              <span>{gatewayState.notice.message}</span>
              <button type="button" onClick={gatewayState.clearNotice}>
                dismiss
              </button>
            </div>
          ) : null}
        </>
      }
    >
      {renderPanel(activeSection, gatewayState)}
    </AppShell>
  );
}

export default App;
