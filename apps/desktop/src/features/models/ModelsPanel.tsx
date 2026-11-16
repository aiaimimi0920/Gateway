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
    <div className="gateway-desktop-panels">
      <Panel className="gateway-desktop-panel">
        <div className="nt-section__head">
          <div>
            <h2>模型目录</h2>
          </div>
          <Button
            variant="primary"
            type="button"
            disabled={state.busy}
            onClick={() => void state.refreshModels()}
          >
            读取模型
          </Button>
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
      </Panel>

      <Panel className="gateway-desktop-panel">
        <DataTable label="模型目录结果" rows={rows} getRowKey={model => model.id}
          columns={[
            { key: "id", header: "Model ID", cell: model => <strong>{model.id}</strong> },
            { key: "object", header: "Object", cell: model => model.object ?? "model" },
            { key: "owner", header: "Owner", cell: model => model.owned_by ?? model.ownedBy ?? "-" },
          ]} empty={<EmptyState title="暂无模型结果" />} />
      </Panel>

      {state.modelsProbe?.data ? (
        <Panel className="gateway-desktop-panel">
          <h3>原始响应</h3>
          <pre className="nt-code">{JSON.stringify(state.modelsProbe.data, null, 2)}</pre>
        </Panel>
      ) : null}
    </div>
  );
}
import { Button, DataTable, EmptyState, Panel } from "@neuro/ui";
