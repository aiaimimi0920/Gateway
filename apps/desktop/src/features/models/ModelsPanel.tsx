import type { GatewayDesktopState, GatewayModelObject } from "../../lib/types";

type ModelsPanelProps = {
  state: GatewayDesktopState;
};

function modelRows(data?: GatewayModelObject[]): GatewayModelObject[] {
  return Array.isArray(data) ? data : [];
}

export function ModelsPanel({ state }: ModelsPanelProps) {
  const rows = modelRows(state.modelsProbe?.data?.data);
  const statusText = state.modelsProbe
    ? state.modelsProbe.ok
      ? `HTTP ${state.modelsProbe.status} / ${state.modelsProbe.durationMs}ms`
      : state.modelsProbe.error ?? `HTTP ${state.modelsProbe.status}`
    : "尚未请求";

  return (
    <div className="nt-stack">
      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// /v1/models</p>
            <h2>模型目录</h2>
          </div>
          <button
            className="nt-btn nt-btn--primary"
            type="button"
            disabled={state.busy}
            onClick={() => void state.refreshModels()}
          >
            读取模型
          </button>
        </div>

        <div className="nt-focus-grid">
          <article className="nt-stat">
            <span>Endpoint</span>
            <strong>{state.baseUrl}/v1/models</strong>
          </article>
          <article className="nt-stat">
            <span>Status</span>
            <strong>{statusText}</strong>
          </article>
          <article className="nt-stat">
            <span>Models</span>
            <strong>{rows.length}</strong>
          </article>
        </div>
      </article>

      <article className="nt-card nt-card--panel">
        {rows.length === 0 ? (
          <p className="nt-empty">
            暂无模型结果。请先启动 Gateway，并确认当前 profile 对应的 route/provider 配置可用。
          </p>
        ) : (
          <div className="nt-table">
            <div className="nt-table__head">
              <span>Model ID</span>
              <span>Object</span>
              <span>Owner</span>
            </div>
            {rows.map((model) => (
              <div className="nt-table__row" key={model.id}>
                <strong>{model.id}</strong>
                <span>{model.object ?? "model"}</span>
                <span>{model.owned_by ?? model.ownedBy ?? "-"}</span>
              </div>
            ))}
          </div>
        )}
      </article>

      {state.modelsProbe?.data ? (
        <article className="nt-card nt-card--panel">
          <p className="nt-kicker">// Raw response</p>
          <pre className="nt-code">{JSON.stringify(state.modelsProbe.data, null, 2)}</pre>
        </article>
      ) : null}
    </div>
  );
}
