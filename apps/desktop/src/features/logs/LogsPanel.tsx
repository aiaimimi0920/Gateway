import type { GatewayDesktopState } from "../../lib/types";

type LogsPanelProps = {
  state: GatewayDesktopState;
};

export function LogsPanel({ state }: LogsPanelProps) {
  const lines = state.logTail.lines;
  const browserPreviewMode = !state.isTauriAvailable;
  const currentLogPath = state.logTail.path ?? state.processSnapshot.logPath ?? null;

  return (
    <div className="nt-stack">
      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Sidecar logs</p>
            <h2>最近启动日志</h2>
          </div>
          <div className="nt-actions">
            <button
              className="nt-btn nt-btn--secondary"
              type="button"
              disabled={state.busy}
              onClick={() => void state.copyDiagnostics()}
            >
              复制诊断
            </button>
            <button
              className="nt-btn nt-btn--outline"
              type="button"
              disabled={state.busy || !currentLogPath}
              onClick={() => void state.copyCurrentLogPath()}
            >
              复制日志路径
            </button>
            <button
              className="nt-btn nt-btn--outline"
              type="button"
              disabled={state.busy || !state.isTauriAvailable}
              onClick={() => void state.openLogDirectory()}
            >
              打开日志目录
            </button>
            <button
              className="nt-btn nt-btn--primary"
              type="button"
              disabled={state.busy || !state.isTauriAvailable}
              onClick={() => void state.refreshLogs()}
            >
              刷新日志
            </button>
          </div>
        </div>
        {browserPreviewMode ? (
          <p className="nt-copy">
            当前是浏览器预览模式。日志面板会显示说明文本，但真实 sidecar 日志读取需要 Tauri
            本地命令。
          </p>
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
      </article>

      {state.diagnosticsReportText ? (
        <article className="nt-card nt-card--panel nt-diagnostics-report">
          <div className="nt-section__head">
            <div>
              <p className="nt-kicker">// Copy fallback</p>
              <h3>脱敏诊断文本</h3>
            </div>
          </div>
          <p className="nt-copy">
            如果系统剪贴板不可用或复制失败，可以从这里手动复制当前生成的诊断文本。
          </p>
          <pre className="nt-code">{state.diagnosticsReportText}</pre>
        </article>
      ) : null}

      <article className="nt-terminal">
        {lines.length === 0 ? (
          <p className="nt-empty">暂无日志。启动 Gateway sidecar 后会显示当前 UI 管理实例的输出。</p>
        ) : (
          lines.map((line, index) => (
            <div className="nt-terminal__line" key={`${index}-${line}`}>
              <span>{String(index + 1).padStart(3, "0")}</span>
              <code>{line}</code>
            </div>
          ))
        )}
      </article>
    </div>
  );
}
