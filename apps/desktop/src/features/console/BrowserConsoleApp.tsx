import { useCallback, useEffect, useMemo, useState } from "react";
import { createGatewayApiClient } from "../../api/client";
import { createConsoleApi, type ConsoleApi } from "../../api/console";
import type {
  ConsoleRouteConfigCommitRequest,
  ConsoleRouteConfigValidationResponse,
  ConsoleRouteDocument,
  ConsoleSecretPatch,
  ConsoleRouteConfigResponse,
  ConsoleRouteRevisionDetailResponse,
  ConsoleRouteRevisionListResponse,
} from "../../api/contracts";
import { SecretConfirmDialog } from "../auth/SecretConfirmDialog";
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

type SecretPatchDraft = {
  operation: ConsoleSecretPatch["operation"];
  value: string;
};

type RouteDocumentDiff = {
  activeAliasCount: number;
  selectedAliasCount: number;
  aliasChanges: Array<{
    alias: string;
    activeModel: string | null;
    selectedModel: string | null;
  }>;
  activeProviderCount: number;
  selectedProviderCount: number;
  addedProviders: string[];
  removedProviders: string[];
  activeModelRouteCount: number;
  selectedModelRouteCount: number;
  addedModelRoutes: string[];
  removedModelRoutes: string[];
};

function createSecretPatchDrafts(
  routeConfig: ConsoleRouteConfigResponse | null,
): Record<string, SecretPatchDraft> {
  return Object.fromEntries(
    (routeConfig?.routeConfig.secrets ?? [])
      .filter((secret) => secret.path.trim().length > 0)
      .map((secret) => [
        secret.path,
        {
          operation: "keep" as const,
          value: "",
        },
      ]),
  );
}

function buildSecretPatches(
  routeConfig: ConsoleRouteConfigResponse | null,
  drafts: Record<string, SecretPatchDraft>,
): ConsoleSecretPatch[] {
  return (routeConfig?.routeConfig.secrets ?? [])
    .filter((secret) => secret.path.trim().length > 0)
    .map((secret) => {
      const draft = drafts[secret.path];
      if (draft?.operation === "replace") {
        return {
          path: secret.path,
          operation: "replace" as const,
          value: draft.value,
        };
      }
      if (draft?.operation === "clear") {
        return {
          path: secret.path,
          operation: "clear" as const,
        };
      }
      return {
        path: secret.path,
        operation: "keep" as const,
      };
    });
}

function diagnosticsList(
  validation: ConsoleRouteConfigValidationResponse | null,
): Array<{ code: string; severity: string; path: string; message: string }> {
  return validation?.validation.diagnostics.diagnostics ?? [];
}

function routeProviderIds(document: ConsoleRouteDocument): string[] {
  return document.providers.map((provider, index) => {
    if (
      typeof provider === "object" &&
      provider !== null &&
      "id" in provider &&
      typeof provider.id === "string"
    ) {
      return provider.id;
    }
    return `provider-${index}`;
  });
}

function routeModelPatterns(document: ConsoleRouteDocument): string[] {
  return document.model_routes.map((route, index) => {
    if (
      typeof route === "object" &&
      route !== null &&
      "pattern" in route &&
      typeof route.pattern === "string"
    ) {
      return route.pattern;
    }
    return `route-${index}`;
  });
}

function compareRouteDocuments(
  active: ConsoleRouteDocument,
  selected: ConsoleRouteDocument,
): RouteDocumentDiff {
  const aliasKeys = new Set([
    ...Object.keys(active.aliases),
    ...Object.keys(selected.aliases),
  ]);
  const activeProviders = routeProviderIds(active);
  const selectedProviders = routeProviderIds(selected);
  const activeRoutes = routeModelPatterns(active);
  const selectedRoutes = routeModelPatterns(selected);

  return {
    activeAliasCount: Object.keys(active.aliases).length,
    selectedAliasCount: Object.keys(selected.aliases).length,
    aliasChanges: [...aliasKeys]
      .sort((left, right) => left.localeCompare(right))
      .map((alias) => ({
        alias,
        activeModel: active.aliases[alias] ?? null,
        selectedModel: selected.aliases[alias] ?? null,
      }))
      .filter((entry) => entry.activeModel !== entry.selectedModel),
    activeProviderCount: activeProviders.length,
    selectedProviderCount: selectedProviders.length,
    addedProviders: selectedProviders.filter((providerId) => !activeProviders.includes(providerId)),
    removedProviders: activeProviders.filter(
      (providerId) => !selectedProviders.includes(providerId),
    ),
    activeModelRouteCount: activeRoutes.length,
    selectedModelRouteCount: selectedRoutes.length,
    addedModelRoutes: selectedRoutes.filter((pattern) => !activeRoutes.includes(pattern)),
    removedModelRoutes: activeRoutes.filter((pattern) => !selectedRoutes.includes(pattern)),
  };
}

function formatRouteDocument(document: ConsoleRouteDocument): string {
  return JSON.stringify(document, null, 2);
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
  const [selectedRevision, setSelectedRevision] = useState<ConsoleRouteRevisionDetailResponse | null>(
    null,
  );
  const [revisionBusy, setRevisionBusy] = useState(false);
  const [secretDrafts, setSecretDrafts] = useState<Record<string, SecretPatchDraft>>({});
  const [secretDialogOpen, setSecretDialogOpen] = useState(false);

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
      setSelectedRevision((current) =>
        current?.routeConfig.revision.id === nextRouteConfig.routeConfig.revision.id
          ? {
              routeConfig: nextRouteConfig.routeConfig,
              active: true,
              hasArchive: current.hasArchive,
            }
          : current,
      );
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
    setSecretDrafts(createSecretPatchDrafts(routeConfig));
  }, [routeConfig?.routeConfig.revision.id]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const secretPatches = useMemo(
    () => buildSecretPatches(routeConfig, secretDrafts),
    [routeConfig, secretDrafts],
  );
  const hasSecretAccess = Boolean(session.session?.secretAccessGranted);
  const selectedRevisionDiff = useMemo(() => {
    if (!routeConfig || !selectedRevision) {
      return null;
    }
    return compareRouteDocuments(
      routeConfig.routeConfig.document,
      selectedRevision.routeConfig.document,
    );
  }, [routeConfig, selectedRevision]);
  const activeRouteDocumentText = useMemo(
    () => (routeConfig ? formatRouteDocument(routeConfig.routeConfig.document) : ""),
    [routeConfig],
  );
  const selectedRouteDocumentText = useMemo(
    () =>
      selectedRevision ? formatRouteDocument(selectedRevision.routeConfig.document) : "",
    [selectedRevision],
  );

  const buildCommitRequest = useCallback(
    (document: ConsoleRouteDocument, messageOverride?: string): ConsoleRouteConfigCommitRequest => {
      if (!routeConfig) {
        throw new Error("Route configuration is not loaded yet.");
      }
      const message = messageOverride?.trim() || commitMessage.trim();
      for (const patch of secretPatches) {
        if (patch.operation !== "keep" && !hasSecretAccess) {
          throw new Error("Secret replacement or clearing requires confirmed secret access.");
        }
        if (patch.operation === "replace" && (!patch.value || patch.value.trim().length === 0)) {
          throw new Error(`Replacement secret for ${patch.path} cannot be empty.`);
        }
      }
      return {
        expectedRevision: routeConfig.routeConfig.revision.id,
        document,
        secretPatches,
        ...(message ? { message } : {}),
      };
    },
    [commitMessage, hasSecretAccess, routeConfig, secretPatches],
  );

  const parseDraft = useCallback((): ConsoleRouteConfigCommitRequest => {
    return buildCommitRequest(parseRouteDocument(editorText));
  }, [buildCommitRequest, editorText]);

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

  const handleInspectRevision = useCallback(
    async (revisionId: string) => {
      if (!managementToken) {
        setError("Gateway management token is unavailable.");
        return;
      }
      setRevisionBusy(true);
      setError(null);
      try {
        const detail = await api.getRouteConfigRevision(managementToken, revisionId);
        setSelectedRevision(detail);
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      } finally {
        setRevisionBusy(false);
      }
    },
    [api, managementToken],
  );

  const loadSelectedRevisionIntoEditor = useCallback(() => {
    if (!selectedRevision) {
      return;
    }
    setEditorText(JSON.stringify(selectedRevision.routeConfig.document, null, 2));
    setCommitMessage(selectedRevision.routeConfig.revision.message ?? "");
    setValidation(null);
  }, [selectedRevision]);

  const handleRestoreSelectedRevision = useCallback(async () => {
    if (!managementToken) {
      setError("Gateway management token is unavailable.");
      return;
    }
    if (!selectedRevision) {
      setError("Select a revision before restoring it.");
      return;
    }
    setActionBusy("save");
    setError(null);
    try {
      const message = `restore revision ${selectedRevision.routeConfig.revision.id}`;
      const draft = buildCommitRequest(selectedRevision.routeConfig.document, message);
      const result = await api.commitRouteConfig(managementToken, draft);
      setRouteConfig({ routeConfig: result.routeConfig });
      setValidation(null);
      setEditorText(JSON.stringify(selectedRevision.routeConfig.document, null, 2));
      setCommitMessage(message);
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setActionBusy(null);
    }
  }, [api, buildCommitRequest, managementToken, refresh, selectedRevision]);

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
                  {routeConfig.routeConfig.secrets.map((secret) => {
                    const draft = secretDrafts[secret.path] ?? {
                      operation: "keep" as const,
                      value: "",
                    };
                    return (
                      <li key={secret.path}>
                        <div className="nt-stack">
                          <div>
                            <strong>{secret.path}</strong>
                            <span>
                              {secret.preview ??
                                (secret.configured ? "configured" : "not configured")}
                            </span>
                          </div>
                          <div className="nt-actions">
                            <button
                              className={`nt-btn${draft.operation === "keep" ? " nt-btn--primary" : " nt-btn--outline"}`}
                              type="button"
                              aria-pressed={draft.operation === "keep"}
                              onClick={() =>
                                setSecretDrafts((current) => ({
                                  ...current,
                                  [secret.path]: { operation: "keep", value: "" },
                                }))
                              }
                            >
                              Keep {secret.path}
                            </button>
                            <button
                              className={`nt-btn${draft.operation === "replace" ? " nt-btn--primary" : " nt-btn--outline"}`}
                              type="button"
                              aria-pressed={draft.operation === "replace"}
                              disabled={!hasSecretAccess}
                              onClick={() =>
                                setSecretDrafts((current) => ({
                                  ...current,
                                  [secret.path]: {
                                    operation: "replace",
                                    value: current[secret.path]?.value ?? "",
                                  },
                                }))
                              }
                            >
                              Replace {secret.path}
                            </button>
                            <button
                              className={`nt-btn${draft.operation === "clear" ? " nt-btn--primary" : " nt-btn--outline"}`}
                              type="button"
                              aria-pressed={draft.operation === "clear"}
                              disabled={!hasSecretAccess}
                              onClick={() =>
                                setSecretDrafts((current) => ({
                                  ...current,
                                  [secret.path]: { operation: "clear", value: "" },
                                }))
                              }
                            >
                              Clear {secret.path}
                            </button>
                          </div>
                          {draft.operation === "replace" ? (
                            <label className="nt-field nt-field--wide">
                              <span>Replacement for {secret.path}</span>
                              <input
                                className="nt-input"
                                type="password"
                                autoComplete="off"
                                value={draft.value}
                                onChange={(event) => {
                                  const { value } = event.currentTarget;
                                  setSecretDrafts((current) => ({
                                    ...current,
                                    [secret.path]: {
                                      operation: "replace",
                                      value,
                                    },
                                  }));
                                }}
                              />
                            </label>
                          ) : null}
                          <span>{draft.operation}</span>
                        </div>
                      </li>
                    );
                  })}
                </ul>
              ) : (
                <p className="nt-empty">当前配置没有需要保留的已登记敏感字段。</p>
              )}
              {secretPatches.length > 0 ? (
                hasSecretAccess ? (
                  <div className="nt-validation-list nt-validation-list--warning">
                    <strong>Secret access active</strong>
                    <ul>
                      <li>
                        当前会话已获得短时 secret grant，可执行 replace / clear 操作。
                      </li>
                      <li>Grant expires at: {session.secretGrant?.expiresAt ?? "unknown"}</li>
                    </ul>
                  </div>
                ) : (
                  <div className="nt-actions">
                    <button
                      className="nt-btn nt-btn--outline"
                      type="button"
                      onClick={() => setSecretDialogOpen(true)}
                    >
                      Confirm secret access
                    </button>
                  </div>
                )
              ) : null}

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
                    <div className="nt-actions nt-actions--right">
                      <span>{entry.active ? "ACTIVE" : entry.source}</span>
                      <button
                        className="nt-btn nt-btn--outline"
                        type="button"
                        disabled={revisionBusy}
                        onClick={() => void handleInspectRevision(entry.revision.id)}
                      >
                        Inspect revision {entry.revision.id}
                      </button>
                    </div>
                  </li>
                ))}
              </ul>
              {selectedRevision ? (
                <div className="nt-stack" aria-label="Selected revision detail">
                  <h3>Revision detail</h3>
                  <dl className="nt-meta-list">
                    <div>
                      <dt>Revision</dt>
                      <dd>{selectedRevision.routeConfig.revision.id}</dd>
                    </div>
                    <div>
                      <dt>Source</dt>
                      <dd>{selectedRevision.routeConfig.source}</dd>
                    </div>
                    <div>
                      <dt>Status</dt>
                      <dd>{selectedRevision.active ? "active" : "archived"}</dd>
                    </div>
                    <div>
                      <dt>Archive</dt>
                      <dd>{selectedRevision.hasArchive ? "available" : "not stored"}</dd>
                    </div>
                  </dl>
                  <h4>Aliases</h4>
                  <ul className="nt-simple-list">
                    {Object.entries(selectedRevision.routeConfig.document.aliases).map(
                      ([alias, model]) => (
                        <li key={`${selectedRevision.routeConfig.revision.id}:${alias}`}>
                          <strong>{alias}</strong>
                          <span>{model}</span>
                        </li>
                      ),
                    )}
                  </ul>
                  <h4>Providers</h4>
                  <ul className="nt-simple-list">
                    {selectedRevision.routeConfig.document.providers.map((provider, index) => {
                      const providerId =
                        typeof provider === "object" &&
                        provider !== null &&
                        "id" in provider &&
                        typeof provider.id === "string"
                          ? provider.id
                          : `provider-${index}`;
                      return <li key={`${selectedRevision.routeConfig.revision.id}:${providerId}`}>{providerId}</li>;
                    })}
                  </ul>
                  {selectedRevisionDiff ? (
                    <div className="nt-validation-list nt-validation-list--warning">
                      <strong>Revision diff summary</strong>
                      <ul>
                        <li>Active aliases: {selectedRevisionDiff.activeAliasCount}</li>
                        <li>Selected aliases: {selectedRevisionDiff.selectedAliasCount}</li>
                        <li>Active providers: {selectedRevisionDiff.activeProviderCount}</li>
                        <li>Selected providers: {selectedRevisionDiff.selectedProviderCount}</li>
                        <li>Active model routes: {selectedRevisionDiff.activeModelRouteCount}</li>
                        <li>Selected model routes: {selectedRevisionDiff.selectedModelRouteCount}</li>
                        {selectedRevisionDiff.aliasChanges.map((entry) => (
                          <li key={`alias-change:${entry.alias}`}>
                            {entry.alias}: {entry.activeModel ?? "<none>"} -&gt;{" "}
                            {entry.selectedModel ?? "<none>"}
                          </li>
                        ))}
                        {selectedRevisionDiff.addedProviders.map((providerId) => (
                          <li key={`provider-added:${providerId}`}>Added provider: {providerId}</li>
                        ))}
                        {selectedRevisionDiff.removedProviders.map((providerId) => (
                          <li key={`provider-removed:${providerId}`}>Removed provider: {providerId}</li>
                        ))}
                        {selectedRevisionDiff.addedModelRoutes.map((pattern) => (
                          <li key={`route-added:${pattern}`}>Added route: {pattern}</li>
                        ))}
                        {selectedRevisionDiff.removedModelRoutes.map((pattern) => (
                          <li key={`route-removed:${pattern}`}>Removed route: {pattern}</li>
                        ))}
                      </ul>
                    </div>
                  ) : null}
                  <div className="nt-grid nt-grid--2" aria-label="Revision document snapshots">
                    <div>
                      <h4>Active route document</h4>
                      <pre className="nt-code" aria-label="Active route document snapshot">
                        {activeRouteDocumentText}
                      </pre>
                    </div>
                    <div>
                      <h4>Selected revision route document</h4>
                      <pre className="nt-code" aria-label="Selected revision route document snapshot">
                        {selectedRouteDocumentText}
                      </pre>
                    </div>
                  </div>
                  <div className="nt-actions nt-actions--right">
                    <button
                      className="nt-btn nt-btn--secondary"
                      type="button"
                      onClick={loadSelectedRevisionIntoEditor}
                    >
                      Load revision into editor
                    </button>
                    <button
                      className="nt-btn nt-btn--primary"
                      type="button"
                      disabled={
                        selectedRevision.active || busy || actionBusy !== null || !mutationSupported
                      }
                      onClick={() => void handleRestoreSelectedRevision()}
                    >
                      Restore revision as active config
                    </button>
                  </div>
                </div>
              ) : (
                <p className="nt-empty">
                  选择任意 revision 可查看归档详情，并将历史配置直接装载到编辑器中。
                </p>
              )}
            </article>
          </section>
        </>
      ) : null}
      <SecretConfirmDialog
        open={secretDialogOpen}
        busy={session.busy}
        error={session.error}
        onOpenChange={setSecretDialogOpen}
        onConfirm={session.confirmSecretAccess}
      />
    </main>
  );
}
