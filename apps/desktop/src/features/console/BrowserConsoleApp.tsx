import { useCallback, useEffect, useMemo, useState } from "react";
import { createGatewayApiClient } from "../../api/client";
import { createConsoleApi, type ConsoleApi } from "../../api/console";
import type {
  ConsoleRouteConfigResponse,
  ConsoleRouteRevisionListResponse,
} from "../../api/contracts";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";

export type BrowserConsoleAppProps = {
  consoleApi?: ConsoleApi;
};

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
    void refresh();
  }, [refresh]);

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
