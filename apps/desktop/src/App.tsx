import { useMemo, useState } from "react";
import { Badge, Button, IconButton, Notice, Panel } from "@neuro/ui";
import {
  Activity,
  Boxes,
  FlaskConical,
  Rocket,
  ScrollText,
  Settings,
  SlidersHorizontal,
  X,
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
import "./App.css";

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
          <Badge tone={running ? "success" : "neutral"}>
            {running ? "RUNNING" : "STANDBY"}
          </Badge>
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
          <Button
            variant={running ? "secondary" : "primary"}
            loading={gatewayState.busy}
            disabled={
              gatewayState.busy ||
              !gatewayState.isTauriAvailable ||
              (!running && !gatewayState.canStartGateway)
            }
            onClick={() => void (running ? gatewayState.stopGateway() : gatewayState.startGateway())}
          >
            {running ? "停止 Gateway" : "启动 Gateway"}
          </Button>
          {gatewayState.startWillSaveDraftProfile ? (
            <Badge tone="warning">启动会先保存当前草稿</Badge>
          ) : null}
        </>
      }
      beforeStage={
        <>
          {browserPreviewMode ? (
            <Notice tone="warning" className="gateway-runtime-notice">
              <span>
                当前处于浏览器预览模式：可以查看界面、测试 HTTP 面板，但本地 sidecar 启停、profile
                持久化和日志读取需要在 Tauri 桌面运行时中使用。
              </span>
            </Notice>
          ) : null}

          <section className="nt-hud-strip gateway-runtime-summary" aria-label="Gateway desktop summary">
            <Panel className="gateway-runtime-stat">
              <span>Runtime</span>
              <strong>{gatewayState.runtimeInfo?.gatewayMode ?? "headless-first"}</strong>
            </Panel>
            <Panel className="gateway-runtime-stat">
              <span>Health</span>
              <strong>{gatewayState.healthProbe?.ok ? "OK" : "CHECK"}</strong>
            </Panel>
            <Panel className="gateway-runtime-stat">
              <span>Ready</span>
              <strong>
                {readinessDegraded ? "DEGRADED" : gatewayState.readyProbe?.ok ? "READY" : "PENDING"}
              </strong>
            </Panel>
          </section>

          {gatewayState.notice ? (
            <Notice tone={gatewayState.notice.tone} className="gateway-runtime-notice">
              <div className="gateway-runtime-notice__content">
                <span>{gatewayState.notice.message}</span>
                <IconButton compact variant="quiet" label="关闭提示" onClick={gatewayState.clearNotice}>
                  <X size={16} aria-hidden="true" />
                </IconButton>
              </div>
            </Notice>
          ) : null}
        </>
      }
    >
      {renderPanel(activeSection, gatewayState)}
    </AppShell>
  );
}

export default App;
