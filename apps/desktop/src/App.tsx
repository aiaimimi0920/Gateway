import { useMemo, useState } from "react";
import { ApiTestPanel } from "./features/api-test/ApiTestPanel";
import { ConfigPanel } from "./features/config/ConfigPanel";
import { LauncherPanel } from "./features/launcher/LauncherPanel";
import { LogsPanel } from "./features/logs/LogsPanel";
import { ModelsPanel } from "./features/models/ModelsPanel";
import { StatusPanel } from "./features/status/StatusPanel";
import type { GatewayDesktopState, SectionId } from "./lib/types";
import { useGatewayDesktopState } from "./state/useGatewayDesktopState";

type NavigationItem = {
  id: SectionId;
  label: string;
  eyebrow: string;
};

const navigationItems: NavigationItem[] = [
  { id: "launcher", label: "Launcher", eyebrow: "本地启动" },
  { id: "config", label: "Config", eyebrow: "运行配置" },
  { id: "status", label: "Status", eyebrow: "健康检查" },
  { id: "models", label: "Models", eyebrow: "模型目录" },
  { id: "api-test", label: "API Test", eyebrow: "接口测试" },
  { id: "logs", label: "Logs", eyebrow: "诊断日志" },
];

const sectionTitles: Record<SectionId, string> = {
  launcher: "本地 Gateway 启动台",
  config: "配置档案与环境变量",
  status: "运行状态与准备度",
  models: "当前模型目录",
  "api-test": "接口调用试验台",
  logs: "最近启动日志",
};

const sectionDescriptions: Record<SectionId, string> = {
  launcher: "面向普通用户的一键启动入口。核心 Gateway 依旧保持无 UI 的 API-first 运行模式。",
  config: "本地 profile 只负责组织环境变量和运行参数，不承载 provider routing 或 pipeline 逻辑。",
  status: "读取 sidecar 进程、/healthz、/readyz 和日志，判断网关是否真正可用。",
  models: "模型列表继续由无 UI Gateway 的 /v1/models 暴露，桌面端只展示和筛选结果。",
  "api-test": "通过 /v1/chat/completions 发起最小调用验证，确认当前本地配置是否生效。",
  logs: "日志只展示本 UI 启动的 Gateway 子进程输出，避免影响外部程序独立运行的实例。",
};

function renderPanel(activeSection: SectionId, state: GatewayDesktopState) {
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
  }
}

function App() {
  const [activeSection, setActiveSection] = useState<SectionId>("launcher");
  const gatewayState = useGatewayDesktopState();

  const activeItem = useMemo(
    () => navigationItems.find((item) => item.id === activeSection) ?? navigationItems[0],
    [activeSection],
  );

  const running = gatewayState.processSnapshot.running;
  const browserPreviewMode = !gatewayState.isTauriAvailable;
  const readinessDegraded = Boolean(gatewayState.readyProbe?.ok && gatewayState.readyProbe?.data?.degraded);

  return (
    <div className="nt-shell">
      <aside className="nt-rail">
        <div className="nt-brand">
          <div className="nt-brand__orb">NG</div>
          <div className="nt-brand__copy">
            <small className="nt-kicker">Gateway</small>
            <strong>Neuro Gateway</strong>
          </div>
        </div>

        <nav className="nt-rail__nav" aria-label="Gateway desktop sections">
          {navigationItems.map((item) => {
            const active = item.id === activeSection;
            return (
              <button
                key={item.id}
                className={`nt-rail__item${active ? " nt-rail__item--active" : ""}`}
                type="button"
                onClick={() => setActiveSection(item.id)}
              >
                <small>{item.eyebrow}</small>
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>

        <div className="nt-rail__footer">
          <div className={running ? "nt-chip nt-chip--online" : "nt-chip"}>{running ? "RUNNING" : "STANDBY"}</div>
          <p>
            核心网关继续保持无 UI 的嵌入式和服务器友好模式，桌面端只是普通用户启动与观察的壳层。
          </p>
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
      </aside>

      <main className="nt-board">
        <header className="nt-board__header">
          <div>
            <p className="nt-kicker">// {activeItem.eyebrow}</p>
            <h1>{sectionTitles[activeSection]}</h1>
            <p className="nt-board__copy">{sectionDescriptions[activeSection]}</p>
          </div>
          <div className="nt-actions nt-actions--right">
            <button
              className="nt-btn nt-btn--secondary"
              type="button"
              disabled={gatewayState.busy}
              onClick={() => void gatewayState.refreshAll()}
            >
              刷新
            </button>
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
          </div>
        </header>

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

        <section className="nt-stage">{renderPanel(activeSection, gatewayState)}</section>
      </main>
    </div>
  );
}

export default App;
