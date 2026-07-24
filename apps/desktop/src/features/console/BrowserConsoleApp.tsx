import { useCallback, useEffect, useMemo, useState } from "react";
import { createGatewayApiClient } from "../../api/client";
import { createConsoleApi, type ConsoleApi } from "../../api/console";
import type {
  ConsoleRouteConfigCommitRequest,
  ConsoleRouteConfigValidationResponse,
  ConsoleRouteDocument,
  ConsoleSecretPatch,
  ConsoleRouteConfigResponse,
  ConsoleRouteRevisionListResponse,
} from "../../api/contracts";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";

export type BrowserConsoleAppProps = {
  consoleApi?: ConsoleApi;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStringRecord(value: unknown): value is Record<string, string> {
  return (
    isRecord(value) &&
    Object.values(value).every((entry) => typeof entry === "string")
  );
}

function isRouteDocument(value: unknown): value is ConsoleRouteDocument {
  return (
    isRecord(value) &&
    Array.isArray(value.providers) &&
    Array.isArray(value.model_routes) &&
    isStringRecord(value.aliases)
  );
}

function parseRouteDocument(text: string): ConsoleRouteDocument {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text) as unknown;
  } catch (error) {
    throw new Error(
      `Route document JSON is invalid: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  if (!isRouteDocument(parsed)) {
    throw new Error(
      "Route document JSON must be an object containing providers[], model_routes[], and aliases{}.",
    );
  }
  return parsed;
}

function buildKeepSecretPatches(routeConfig: ConsoleRouteConfigResponse | null): ConsoleSecretPatch[] {
  return (routeConfig?.routeConfig.secrets ?? [])
    .filter((secret) => secret.path.trim().length > 0)
    .map((secret) => ({
      path: secret.path,
      operation: "keep" as const,
    }));
}

function diagnosticsList(
  validation: ConsoleRouteConfigValidationResponse | null,
): Array<{ code: string; severity: string; path: string; message: string }> {
  return validation?.validation.diagnostics.diagnostics ?? [];
}

export function BrowserConsoleApp({ consoleApi }: BrowserConsoleAppProps) {
  const host = useGatewayHost();
  const session = useManagementSession();
  const client = useMemo(() => createGatewayApiClient({ host }), [host]);
  const api = useMemo(() => consoleApi ?? createConsoleApi(client), [client, consoleApi]);
  const managementToken = session.managementToken;
  const [routeConfig, setRouteConfig] = useState<ConsoleRouteConfigResponse | null>(null);
  const [revisions, setRevisions] = useState<ConsoleRouteRevisionListResponse | null>(null);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editorText, setEditorText] = useState("");
  const [commitMessage, setCommitMessage] = useState("");
  const [validation, setValidation] = useState<ConsoleRouteConfigValidationResponse | null>(null);
  const [actionBusy, setActionBusy] = useState<"validate" | "save" | null>(null);

  const refresh = useCallback(async () => {
    if (!managementToken) {
      setError("Gateway management token is unavailable.");
      setBusy(false);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const [nextRouteConfig, nextRevisions] = await Promise.all([
        api.getRouteConfig(managementToken),
        api.listRouteConfigRevisions(managementToken),
      ]);
      setRouteConfig(nextRouteConfig);
      setRevisions(nextRevisions);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }, [api, managementToken]);

  useEffect(() => {
    if (!routeConfig) {
      return;
    }
    setEditorText(JSON.stringify(routeConfig.routeConfig.document, null, 2));
    setCommitMessage(routeConfig.routeConfig.revision.message ?? "");
  }, [routeConfig?.routeConfig.revision.id]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const secretPatches = useMemo(() => buildKeepSecretPatches(routeConfig), [routeConfig]);

  const parseDraft = useCallback((): ConsoleRouteConfigCommitRequest => {
    if (!routeConfig) {
      throw new Error("Route configuration is not loaded yet.");
    }
    const message = commitMessage.trim();
    return {
      expectedRevision: routeConfig.routeConfig.revision.id,
      document: parseRouteDocument(editorText),
      secretPatches,
      ...(message ? { message } : {}),
    };
  }, [commitMessage, editorText, routeConfig, secretPatches]);

  const handleValidate = useCallback(async () => {
    if (!managementToken) {
      setError("Gateway management token is unavailable.");
      return;
    }
    setActionBusy("validate");
    setError(null);
    try {
      const draft = parseDraft();
      const result = await api.validateRouteConfig(managementToken, {
        document: draft.document,
        secretPatches: draft.secretPatches,
      });
      setValidation(result);
    } catch (cause) {
      setValidation(null);
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setActionBusy(null);
    }
  }, [api, managementToken, parseDraft]);

  const handleSave = useCallback(async () => {
    if (!managementToken) {
      setError("Gateway management token is unavailable.");
      return;
    }
    setActionBusy("save");
    setError(null);
    try {
      const draft = parseDraft();
      const result = await api.commitRouteConfig(managementToken, draft);
      setRouteConfig({ routeConfig: result.routeConfig });
      setValidation(null);
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setActionBusy(null);
    }
  }, [api, managementToken, parseDraft, refresh]);

  const validationDiagnostics = diagnosticsList(validation);
  const mutationSupported = routeConfig?.routeConfig.mutationSupported ?? false;

  if (busy && !routeConfig) {
    return <main role="status">Loading Gateway console...</main>;
  }

  return (
    <main className="nt-board nt-board--browser-console">
      <header className="nt-board__header">
        <div>
          <p className="nt-kicker">// Gateway Console</p>
          <h1>Gateway Web Console</h1>
          <p className="nt-board__copy">
            直接通过 Gateway 本体托管的浏览器控制台，当前接入 route-config 与 revision
            历史能力。
          </p>
        </div>
        <div className="nt-actions nt-actions--right">
          <button className="nt-btn nt-btn--secondary" type="button" onClick={() => void refresh()}>
            刷新
          </button>
          <button className="nt-btn nt-btn--outline" type="button" onClick={() => void session.logout()}>
            退出
          </button>
        </div>
      </header>

      {error ? (
        <div className="nt-alert nt-alert--danger" role="alert">
          <span>{error}</span>
        </div>
      ) : null}

      {routeConfig ? (
        <>
          <section className="nt-hud-strip" aria-label="Gateway console summary">
            <article className="nt-card nt-card--stat">
              <span>Active revision</span>
              <strong>{routeConfig.routeConfig.revision.id}</strong>
            </article>
            <article className="nt-card nt-card--stat">
              <span>Source</span>
              <strong>{routeConfig.routeConfig.source}</strong>
            </article>
            <article className="nt-card nt-card--stat">
              <span>Providers</span>
              <strong>{routeConfig.routeConfig.document.providers.length}</strong>
            </article>
            <article className="nt-card nt-card--stat">
              <span>Model routes</span>
              <strong>{routeConfig.routeConfig.document.model_routes.length}</strong>
            </article>
          </section>

          <section className="nt-grid nt-grid--2">
            <article className="nt-card nt-card--panel">
              <div className="nt-section__head">
                <div>
                  <p className="nt-kicker">// Active config</p>
                  <h2>当前路由配置</h2>
                </div>
              </div>
              <dl className="nt-meta-list">
                <div>
                  <dt>Revision</dt>
                  <dd>{routeConfig.routeConfig.revision.id}</dd>
                </div>
                <div>
                  <dt>Sequence</dt>
                  <dd>{routeConfig.routeConfig.revision.sequence}</dd>
                </div>
                <div>
                  <dt>Mutation</dt>
                  <dd>{routeConfig.routeConfig.mutationSupported ? "supported" : "read-only"}</dd>
                </div>
                <div>
                  <dt>Repair</dt>
                  <dd>{routeConfig.routeConfig.requiresRepair ? "required" : "not required"}</dd>
                </div>
              </dl>

              <h3>Aliases</h3>
              <ul className="nt-simple-list">
                {Object.entries(routeConfig.routeConfig.document.aliases).map(([alias, model]) => (
                  <li key={alias}>
                    <strong>{alias}</strong>
                    <span>{model}</span>
                  </li>
                ))}
              </ul>

              <h3>Providers</h3>
              <ul className="nt-simple-list">
                {routeConfig.routeConfig.document.providers.map((provider, index) => {
                  const providerId =
                    typeof provider === "object" &&
                    provider !== null &&
                    "id" in provider &&
                    typeof provider.id === "string"
                      ? provider.id
                      : `provider-${index}`;
                  return <li key={providerId}>{providerId}</li>;
                })}
              </ul>
            </article>

            <article className="nt-card nt-card--panel">
              <div className="nt-section__head">
                <div>
                  <p className="nt-kicker">// Route editor</p>
                  <h2>路由配置编辑器</h2>
                </div>
                <div className="nt-actions nt-actions--right">
                  <button
                    className="nt-btn nt-btn--secondary"
                    type="button"
                    disabled={busy || actionBusy !== null || !mutationSupported}
                    onClick={() => void handleValidate()}
                  >
                    Validate draft
                  </button>
                  <button
                    className="nt-btn nt-btn--primary"
                    type="button"
                    disabled={busy || actionBusy !== null || !mutationSupported}
                    onClick={() => void handleSave()}
                  >
                    Save route config
                  </button>
                </div>
              </div>

              <label className="nt-field nt-field--wide">
                <span>Revision message</span>
                <input
                  className="nt-input"
                  placeholder="可选：写入 revision history 的说明"
                  value={commitMessage}
                  onChange={(event) => setCommitMessage(event.currentTarget.value)}
                />
              </label>

              <label className="nt-field nt-field--wide">
                <span>Route document JSON</span>
                <textarea
                  className="nt-input nt-textarea"
                  value={editorText}
                  spellCheck={false}
                  onChange={(event) => {
                    setEditorText(event.currentTarget.value);
                    setValidation(null);
                    if (error?.startsWith("Route document JSON")) {
                      setError(null);
                    }
                  }}
                />
              </label>

              <div className="nt-copy" aria-label="Route editor secret handling">
                当前会自动为已存在的敏感字段生成 keep patch，避免浏览器编辑时覆盖现有密钥。
              </div>
              {secretPatches.length > 0 ? (
                <ul className="nt-simple-list">
                  {secretPatches.map((patch) => (
                    <li key={patch.path}>
                      <strong>{patch.path}</strong>
                      <span>{patch.operation}</span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="nt-empty">当前配置没有需要保留的已登记敏感字段。</p>
              )}

              {!mutationSupported ? (
                <div className="nt-validation-list nt-validation-list--warning">
                  <strong>当前实例是只读模式</strong>
                  <ul>
                    <li>route-config runtime 没有启用写入支持，浏览器控制台暂时只能查看。</li>
                  </ul>
                </div>
              ) : null}

              {validation ? (
                validationDiagnostics.length > 0 ? (
                  <div className="nt-validation-list" role="alert">
                    <strong>Draft validation reported diagnostics</strong>
                    <ul>
                      {validationDiagnostics.map((item) => (
                        <li key={`${item.code}:${item.path}:${item.message}`}>
                          [{item.severity}] {item.path} · {item.message}
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : (
                  <div className="nt-validation-list nt-validation-list--warning">
                    <strong>Draft validation passed</strong>
                    <ul>
                      <li>
                        Candidate revision is structurally valid and preserves configured secrets via
                        keep patches.
                      </li>
                    </ul>
                  </div>
                )
              ) : null}
            </article>

            <article className="nt-card nt-card--panel">
              <div className="nt-section__head">
                <div>
                  <p className="nt-kicker">// Revision history</p>
                  <h2>修订历史</h2>
                </div>
              </div>
              <ul className="nt-simple-list">
                {revisions?.revisions.map((entry) => (
                  <li key={entry.revision.id}>
                    <div>
                      <strong>{entry.revision.id}</strong>
                      <span>{entry.revision.message ?? "no message"}</span>
                    </div>
                    <span>{entry.active ? "ACTIVE" : entry.source}</span>
                  </li>
                ))}
              </ul>
            </article>
          </section>
        </>
      ) : null}
    </main>
  );
}
