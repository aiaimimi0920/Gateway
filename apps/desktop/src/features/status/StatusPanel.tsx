import type { GatewayDesktopState, GatewayHttpProbe } from "../../lib/types";

type StatusPanelProps = {
  state: GatewayDesktopState;
};

function degradedFlag(data: unknown): boolean {
  if (!data || typeof data !== "object") {
    return false;
  }
  return (data as { degraded?: unknown }).degraded === true;
}

function statusLabel(probe?: GatewayHttpProbe<unknown>): string {
  if (!probe) {
    return "pending";
  }
  if (probe.ok && degradedFlag(probe.data)) {
    return `degraded mode / HTTP ${probe.status}`;
  }
  if (probe.ok) {
    return `HTTP ${probe.status}`;
  }
  return probe.status === 0 ? "network error" : `HTTP ${probe.status}`;
}

function isDegradedProbe(probe?: GatewayHttpProbe<unknown>): boolean {
  return Boolean(probe?.ok && degradedFlag(probe.data));
}

function ProbeCard({
  title,
  endpoint,
  probe,
}: {
  title: string;
  endpoint: string;
  probe?: GatewayHttpProbe<unknown>;
}) {
  const degraded = isDegradedProbe(probe);
  const ok = (probe?.ok ?? false) && !degraded;
  const toneClass = degraded
    ? "nt-badge--warning"
    : ok
      ? "nt-badge--success"
      : "nt-badge--warning";
  return (
    <article className="nt-card">
      <div className="nt-row nt-row--between">
        <div>
          <p className="nt-kicker">// {endpoint}</p>
          <h3>{title}</h3>
        </div>
        <span className={`nt-badge ${toneClass}`}>
          {statusLabel(probe)}
        </span>
      </div>
      <dl className="nt-meta-list nt-meta-list--inline">
        <div>
          <dt>Latency</dt>
          <dd>{probe ? `${probe.durationMs}ms` : "-"}</dd>
        </div>
        <div>
          <dt>Error</dt>
          <dd>{probe?.error ?? "-"}</dd>
        </div>
        <div>
          <dt>Mode</dt>
          <dd>{degraded ? "degraded mode" : probe?.ok ? "normal" : "-"}</dd>
        </div>
      </dl>
      <pre className="nt-code">{JSON.stringify(probe?.data ?? null, null, 2)}</pre>
    </article>
  );
}

export function StatusPanel({ state }: StatusPanelProps) {
  const snapshot = state.processSnapshot;

  return (
    <div className="nt-stack">
      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Runtime status</p>
            <h2>运行状态与准备度</h2>
          </div>
          <button
            className="nt-btn nt-btn--primary"
            type="button"
            disabled={state.busy}
            onClick={() => void state.refreshRuntimeSignals()}
          >
            刷新检查
          </button>
        </div>

        <div className="nt-focus-grid">
          <article className="nt-stat">
            <span>Sidecar</span>
            <strong>{snapshot.running ? "RUNNING" : "STOPPED"}</strong>
          </article>
          <article className="nt-stat">
            <span>Base URL</span>
            <strong>{state.baseUrl}</strong>
          </article>
          <article className="nt-stat">
            <span>PID</span>
            <strong>{snapshot.pid ?? "-"}</strong>
          </article>
          <article className="nt-stat">
            <span>Log path</span>
            <strong>{snapshot.logPath ?? "-"}</strong>
          </article>
          <article className="nt-stat">
            <span>Startup</span>
            <strong>{snapshot.startupState ?? "-"}</strong>
          </article>
          <article className="nt-stat">
            <span>Shutdown</span>
            <strong>{snapshot.shutdownState ?? "-"}</strong>
          </article>
        </div>
        {snapshot.lastError ? <p className="nt-copy">{snapshot.lastError}</p> : null}
      </article>

      <div className="nt-grid nt-grid--2">
        <ProbeCard endpoint="/healthz" title="Liveness" probe={state.healthProbe} />
        <ProbeCard endpoint="/readyz" title="Readiness" probe={state.readyProbe} />
      </div>
    </div>
  );
}
