import type { GatewayDesktopState } from "../../lib/types";

type ApiTestPanelProps = {
  state: GatewayDesktopState;
};

export function ApiTestPanel({ state }: ApiTestPanelProps) {
  const input = state.apiTestInput;
  const result = state.apiTestResult;

  return (
    <div className="nt-stack">
      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// /v1/chat/completions</p>
            <h2>接口调用试验台</h2>
          </div>
          <button
            className="nt-btn nt-btn--primary"
            type="button"
            disabled={state.busy || input.model.trim().length === 0}
            onClick={() => void state.runApiTest()}
          >
            发送测试请求
          </button>
        </div>

        <div className="nt-form-grid">
          <label className="nt-field nt-field--wide">
            <span>Gateway API Key</span>
            <input
              className="nt-input"
              placeholder="可选：如果 Gateway 已配置访问密钥，请填写"
              type="password"
              value={input.apiKey}
              onChange={(event) =>
                state.updateApiTestInput({ ...input, apiKey: event.currentTarget.value })
              }
            />
          </label>
          <label className="nt-field">
            <span>Model</span>
            <input
              className="nt-input"
              value={input.model}
              onChange={(event) =>
                state.updateApiTestInput({ ...input, model: event.currentTarget.value })
              }
            />
          </label>
          <label className="nt-field nt-field--wide">
            <span>Message</span>
            <textarea
              className="nt-input nt-textarea"
              value={input.message}
              onChange={(event) =>
                state.updateApiTestInput({ ...input, message: event.currentTarget.value })
              }
            />
          </label>
        </div>
      </article>

      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Response</p>
            <h2>测试响应</h2>
          </div>
          {result ? (
            <span className={`nt-badge ${result.ok ? "nt-badge--success" : "nt-badge--warning"}`}>
              {result.status || "network"} / {result.durationMs}ms
            </span>
          ) : null}
        </div>
        {result ? (
          <>
            <div className="nt-api-context" aria-label="API test request context">
              <span>Profile: {result.profileName}</span>
              <span>Base URL: {result.baseUrl}</span>
              <span>Model: {result.model}</span>
              <span>Requested: {result.requestedAt}</span>
            </div>
            <p className="nt-copy">Endpoint: {result.endpoint}</p>
            <pre className="nt-code">
              {JSON.stringify(result.data ?? { error: result.error ?? "empty response" }, null, 2)}
            </pre>
          </>
        ) : (
          <p className="nt-empty">尚未发送请求。这里会展示 Gateway 公共 API 返回的真实响应。</p>
        )}
      </article>
    </div>
  );
}
