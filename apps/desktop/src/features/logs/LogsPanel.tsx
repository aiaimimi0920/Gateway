import type { GatewayDesktopState } from "../../lib/types";

type LogsPanelProps = {
  state: GatewayDesktopState;
};

export function LogsPanel({ state }: LogsPanelProps) {
  const lines = state.logTail.lines;
  const browserPreviewMode = !state.isTauriAvailable;
  const currentLogPath = state.logTail.path ?? state.processSnapshot.logPath ?? null;

  return (
    <div className="gateway-desktop-panels">
      <Panel className="gateway-desktop-panel">
        <div className="nt-section__head">
          <div>
            <h2>最近启动日志</h2>
          </div>
          <div className="nt-actions">
            <Button
              variant="secondary"
              type="button"
              disabled={state.busy}
              onClick={() => void state.copyDiagnostics()}
            >
              复制诊断
            </Button>
            <Button
              variant="quiet"
              type="button"
              disabled={state.busy || !currentLogPath}
              onClick={() => void state.copyCurrentLogPath()}
            >
              复制日志路径
            </Button>
            <Button
              variant="quiet"
              type="button"
              disabled={state.busy || !state.isTauriAvailable}
              onClick={() => void state.openLogDirectory()}
            >
              打开日志目录
            </Button>
            <Button
              variant="primary"
              type="button"
              disabled={state.busy || !state.isTauriAvailable}
              onClick={() => void state.refreshLogs()}
            >
              刷新日志
            </Button>
          </div>
        </div>
        {browserPreviewMode ? (
          <Notice tone="warning">浏览器预览不能读取本地 sidecar 日志。</Notice>
        ) : null}
        <dl className="nt-meta-list nt-meta-list--inline">
          <div>
            <dt>Path</dt>
            <dd>{currentLogPath ?? "尚无 sidecar 日志路径"}</dd>
          </div>
          <div>
            <dt>Lines</dt>
            <dd>{lines.length}</dd>
          </div>
        </dl>
      </Panel>

      {state.diagnosticsReportText ? (
        <Panel className="gateway-desktop-panel">
          <div className="nt-section__head">
            <div>
              <h3>脱敏诊断文本</h3>
            </div>
          </div>
          <pre className="nt-code">{state.diagnosticsReportText}</pre>
        </Panel>
      ) : null}

      <Panel className="gateway-desktop-terminal" aria-label="启动日志内容">
        {lines.length === 0 ? (
          <EmptyState title="暂无日志" />
        ) : (
          lines.map((line, index) => (
            <div className="gateway-desktop-terminal__line" key={`${index}-${line}`}>
              <span>{String(index + 1).padStart(3, "0")}</span>
              <code>{line}</code>
            </div>
          ))
        )}
      </Panel>
    </div>
  );
}
import { Button, EmptyState, Notice, Panel } from "@neuro/ui";
