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
    <div className="gateway-desktop-panels">
      <Panel className="gateway-desktop-panel gateway-launcher-summary">
        <div className="gateway-launcher-actions">
          <h2>Gateway</h2>
          {state.hasUnsavedProfileChanges ? (
            <Badge tone="warning">未保存变更</Badge>
          ) : null}
          <div className="nt-actions">
            <Button
              variant="primary"
              type="button"
              disabled={state.busy || running || !state.canStartGateway}
              onClick={() => void state.startGateway()}
            >
              启动 Gateway
            </Button>
            <Button
              variant="secondary"
              type="button"
              disabled={state.busy || !running || !state.isTauriAvailable}
              onClick={() => void state.stopGateway()}
            >
              停止 sidecar
            </Button>
            <Button
              variant="quiet"
              type="button"
              disabled={state.busy}
              onClick={() => void state.refreshAll()}
            >
              刷新状态
            </Button>
          </div>
          {state.startWillSaveDraftProfile ? (
            <Notice>启动会先保存当前草稿。</Notice>
          ) : null}
          {browserPreviewMode ? (
            <Notice tone="warning">浏览器预览不能启停本地 sidecar。</Notice>
          ) : null}
        </div>

        <aside className="gateway-launcher-actions">
          <Badge tone={running ? "success" : "neutral"}>
            {running ? "Sidecar running" : "Sidecar stopped"}
          </Badge>
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
      </Panel>

      {processSnapshot.lastError ? (
        <Panel className="gateway-desktop-panel">
          <h3>启动诊断</h3>
          <Notice tone="danger">{processSnapshot.lastError}</Notice>
          {recentLogLines.length > 0 ? (
            <pre className="nt-code">{recentLogLines.join("\n")}</pre>
          ) : (
            <EmptyState title="当前没有可用的最近日志" />
          )}
        </Panel>
      ) : null}

      <Panel className="gateway-desktop-panel">
        <div className="nt-section__head">
          <div>
            <h3>首次启动引导</h3>
          </div>
        </div>
        <div className="nt-onboarding-list">
          {state.onboardingSteps.map((step, index) => (
            <div className="gateway-onboarding-step" key={step.id}>
              <span>{String(index + 1).padStart(2, "0")}</span>
              <div>
                <strong>{step.title}</strong>
                <p>{step.description}</p>
              </div>
              <Badge tone={step.status === "done" ? "success" : step.status === "warning" ? "warning" : "neutral"}>{step.status}</Badge>
            </div>
          ))}
        </div>
      </Panel>
    </div>
  );
}
import { Badge, Button, EmptyState, Notice, Panel } from "@neuro/ui";
