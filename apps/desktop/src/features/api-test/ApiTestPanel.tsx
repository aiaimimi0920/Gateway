import type { GatewayDesktopState } from "../../lib/types";

type ApiTestPanelProps = {
  state: GatewayDesktopState;
};

export function ApiTestPanel({ state }: ApiTestPanelProps) {
  const input = state.apiTestInput;
  const result = state.apiTestResult;

  return (
    <div className="gateway-desktop-panels">
      <Panel className="gateway-desktop-panel">
        <div className="nt-section__head">
          <div>
            <h2>接口调用试验台</h2>
          </div>
          <Button
            variant="primary"
            type="button"
            disabled={state.busy || input.model.trim().length === 0}
            onClick={() => void state.runApiTest()}
          >
            发送测试请求
          </Button>
        </div>

        <div className="gateway-profile-fields">
          <div className="gateway-profile-fields__wide"><Field label="Gateway API Key">
            <Input
              placeholder="可选：如果 Gateway 已配置访问密钥，请填写"
              type="password"
              autoComplete="off"
              value={input.apiKey}
              onChange={(event) =>
                state.updateApiTestInput({ ...input, apiKey: event.currentTarget.value })
              }
            />
          </Field></div>
          <Field label="Model">
            <Input
              value={input.model}
              onChange={(event) =>
                state.updateApiTestInput({ ...input, model: event.currentTarget.value })
              }
            />
          </Field>
          <div className="gateway-profile-fields__wide"><Field label="Message">
            <Textarea
              value={input.message}
              onChange={(event) =>
                state.updateApiTestInput({ ...input, message: event.currentTarget.value })
              }
            />
          </Field></div>
        </div>
      </Panel>

      <Panel className="gateway-desktop-panel">
        <div className="nt-section__head">
          <div>
            <h2>测试响应</h2>
          </div>
          {result ? (
            <Badge tone={result.ok ? "success" : "warning"}>
              {result.status || "network"} / {result.durationMs}ms
            </Badge>
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
          <EmptyState title="尚未发送请求" />
        )}
      </Panel>
    </div>
  );
}
import { Badge, Button, EmptyState, Field, Input, Panel, Textarea } from "@neuro/ui";
