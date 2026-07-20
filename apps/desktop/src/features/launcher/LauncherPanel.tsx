import type { GatewayDesktopState } from "../../lib/types";

type LauncherPanelProps = {
  state: GatewayDesktopState;
};

function formatValue(value?: string | number | null): string {
  if (value === undefined || value === null || value === "") {
    return "-";
  }
  return String(value);
}

export function LauncherPanel({ state }: LauncherPanelProps) {
  const { processSnapshot, draftProfile } = state;
  const running = processSnapshot.running;
  const browserPreviewMode = !state.isTauriAvailable;
  const recentLogLines = processSnapshot.recentLogLines ?? [];

  return (
    <div className="nt-stack">
      <article className="nt-card nt-card--hero">
        <div className="nt-card__body">
          <p className="nt-kicker">// Local launcher</p>
          <h2>无 UI Gateway 仍是核心，桌面端只负责启动、观察和测试。</h2>
          {state.hasUnsavedProfileChanges ? (
            <span className="nt-dirty-badge">Unsaved changes</span>
          ) : null}
          <p>
            普通用户从这里启动本地 sidecar；嵌入式和服务器场景继续直接运行
            <code>neuro-gateway.exe</code>。桌面端不会复制 provider routing、credential
            pipeline 或请求调度逻辑。
          </p>
          <div className="nt-actions">
            <button
              className="nt-btn nt-btn--primary"
              type="button"
              disabled={state.busy || running || !state.canStartGateway}
              onClick={() => void state.startGateway()}
            >
              启动 Gateway
            </button>
            <button
              className="nt-btn nt-btn--secondary"
              type="button"
              disabled={state.busy || !running || !state.isTauriAvailable}
              onClick={() => void state.stopGateway()}
            >
              停止 sidecar
            </button>
            <button
              className="nt-btn nt-btn--outline"
              type="button"
              disabled={state.busy}
              onClick={() => void state.refreshAll()}
            >
              刷新状态
            </button>
          </div>
          {state.startWillSaveDraftProfile ? (
            <p className="nt-start-save-hint">启动会先保存当前草稿，然后再启动 sidecar。</p>
          ) : null}
          {browserPreviewMode ? (
            <p className="nt-copy">
              当前是浏览器预览模式。本地 sidecar 启停只能在 Tauri 桌面运行时中使用；这里保留
              HTTP 状态观察，方便继续调样式和接口面板。
            </p>
          ) : null}
        </div>

        <aside className="nt-status-card">
          <div className={`nt-status-pill ${running ? "nt-status-pill--online" : "nt-status-pill--offline"}`}>
            <span className="nt-status-dot" />
            {running ? "Sidecar running" : "Sidecar stopped"}
          </div>
          <dl className="nt-meta-list">
            <div>
              <dt>Profile</dt>
              <dd>{formatValue(processSnapshot.profileName ?? draftProfile.name)}</dd>
            </div>
            <div>
              <dt>PID</dt>
              <dd>{formatValue(processSnapshot.pid)}</dd>
            </div>
            <div>
              <dt>Port</dt>
              <dd>{formatValue(processSnapshot.port ?? draftProfile.port)}</dd>
            </div>
            <div>
              <dt>Mode</dt>
              <dd>{draftProfile.runtimeRole}</dd>
            </div>
            <div>
              <dt>Startup</dt>
              <dd>{formatValue(processSnapshot.startupState)}</dd>
            </div>
            <div>
              <dt>Shutdown</dt>
              <dd>{formatValue(processSnapshot.shutdownState)}</dd>
            </div>
          </dl>
        </aside>
      </article>

      {processSnapshot.lastError ? (
        <article className="nt-card nt-card--panel">
          <p className="nt-kicker">// Startup diagnostic</p>
          <h3>启动诊断</h3>
          <p className="nt-copy">{processSnapshot.lastError}</p>
          {recentLogLines.length > 0 ? (
            <pre className="nt-code">{recentLogLines.join("\n")}</pre>
          ) : (
            <p className="nt-empty">当前没有可用的最近日志。</p>
          )}
        </article>
      ) : null}

      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// First run</p>
            <h3>首次启动引导</h3>
          </div>
        </div>
        <div className="nt-onboarding-list">
          {state.onboardingSteps.map((step, index) => (
            <div className={`nt-onboarding-step nt-onboarding-step--${step.status}`} key={step.id}>
              <span>{String(index + 1).padStart(2, "0")}</span>
              <div>
                <strong>{step.title}</strong>
                <p>{step.description}</p>
              </div>
              <em>{step.status}</em>
            </div>
          ))}
        </div>
      </article>

      <div className="nt-card-grid">
        <article className="nt-card">
          <p className="nt-kicker">// Runtime</p>
          <h3>Headless-first</h3>
          <p>桌面版以子进程方式启动同一个核心二进制，保证 API 流程仍由 Gateway 本体提供。</p>
        </article>
        <article className="nt-card">
          <p className="nt-kicker">// Local profile</p>
          <h3>环境变量渲染</h3>
          <p>profile 只在启动时转换为环境变量，不把桌面端变成第二套配置真相。</p>
        </article>
        <article className="nt-card">
          <p className="nt-kicker">// Diagnostics</p>
          <h3>状态与日志一屏可见</h3>
          <p>健康检查、ready 状态、模型目录和日志都通过 Gateway 自己的接口或 Tauri 本地命令读取。</p>
        </article>
      </div>
    </div>
  );
}
