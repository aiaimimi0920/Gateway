import type {
  GatewayDependencyCheckItem,
  GatewayDesktopState,
  GatewayEnvEntry,
  GatewayProfile,
  GatewayProfilePathCheckItem,
} from "../../lib/types";
import { PROFILE_TEMPLATES } from "../../lib/profileTemplates";

type ConfigPanelProps = {
  state: GatewayDesktopState;
};

type TextFieldKey = Exclude<keyof GatewayProfile, "port" | "extraEnv">;

function optionalText(value?: string | null): string {
  return value ?? "";
}

function renderPathCheckItem(label: string, item?: GatewayProfilePathCheckItem) {
  if (!item) {
    return null;
  }

  return (
    <div className={`nt-path-check__item${item.ok ? " nt-path-check__item--ok" : ""}`}>
      <span>{label}</span>
      <strong>{item.ok ? "OK" : "CHECK"}</strong>
      <code>{item.resolvedPath || "(not configured)"}</code>
      <small>
        {item.message} · expected {item.expectedKind} · exists {String(item.exists)}
      </small>
    </div>
  );
}

function renderDependencyCheck(item?: GatewayDependencyCheckItem) {
  if (!item) {
    return null;
  }
  return (
    <div className={`nt-path-check__item${item.ok ? " nt-path-check__item--ok" : ""}`}>
      <span>{item.name}</span>
      <strong>{item.ok ? "OK" : item.required ? "REQUIRED" : "CHECK"}</strong>
      <small>{item.message}</small>
    </div>
  );
}

export function ConfigPanel({ state }: ConfigPanelProps) {
  const profile = state.draftProfile;
  const browserPreviewMode = !state.isTauriAvailable;
  const validation = state.profileValidation;
  const fieldErrors = validation.fieldErrors;

  const patchProfile = (patch: Partial<GatewayProfile>) => {
    state.updateDraftProfile({ ...profile, ...patch });
  };

  const updateText = (key: TextFieldKey, value: string) => {
    patchProfile({ [key]: value } as Partial<GatewayProfile>);
  };

  const updateEnvEntry = (index: number, patch: Partial<GatewayEnvEntry>) => {
    patchProfile({
      extraEnv: profile.extraEnv.map((entry, entryIndex) =>
        entryIndex === index ? { ...entry, ...patch } : entry,
      ),
    });
  };

  return (
    <div className="nt-stack">
      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <h2>Profile 快速模板</h2>
          </div>
        </div>
        <p className="nt-copy">
          模板只会更新当前草稿，不会自动保存 profile，也不会启动 Gateway。应用后可继续编辑、
          检查路径，再手动保存。
        </p>
        <div className="nt-template-grid">
          {PROFILE_TEMPLATES.map((template) => (
            <button
              className="nt-template-card"
              key={template.id}
              type="button"
              disabled={state.busy}
              onClick={() => state.applyProfileTemplate(template.id)}
            >
              <span>{template.title}</span>
              <small>{template.description}</small>
            </button>
          ))}
        </div>
      </article>

      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <h2>本地运行配置</h2>
            {state.hasUnsavedProfileChanges ? (
              <span className="nt-dirty-badge">未保存变更</span>
            ) : null}
          </div>
          <div className="nt-actions">
            <select
              className="nt-select"
              value={state.selectedProfileName}
              disabled={!state.isTauriAvailable}
              onChange={(event) => void state.selectProfile(event.currentTarget.value)}
            >
              {state.profileNames.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
            <button
              className="nt-btn nt-btn--primary"
              type="button"
              disabled={state.busy || !state.canSaveProfile}
              onClick={() => void state.saveDraftProfile()}
            >
              保存 profile
            </button>
            <button
              className="nt-btn nt-btn--outline"
              type="button"
              disabled={state.busy || !state.isTauriAvailable}
              onClick={() => void state.deleteSelectedProfile()}
            >
              删除/重置
            </button>
            <button
              className="nt-btn nt-btn--secondary"
              type="button"
              disabled={state.busy || !state.isTauriAvailable}
              onClick={() => void state.checkProfilePaths()}
            >
              检查路径
            </button>
          </div>
        </div>

        {browserPreviewMode ? (
          <p className="nt-copy">
            浏览器预览模式下只保留表单预览；profile 的读取、保存、删除和持久化需要在 Tauri
            桌面运行时中完成。
          </p>
        ) : null}

        {state.profileValidation.errors.length > 0 ? (
          <div className="nt-validation-list" role="alert">
            <strong>配置需要修正</strong>
            <ul>
              {validation.errors.map((error) => (
                <li key={error}>{error}</li>
              ))}
            </ul>
          </div>
        ) : validation.warnings.length > 0 ? (
          <div className="nt-validation-list nt-validation-list--warning">
            <strong>保存前后端会继续检查</strong>
            <ul>
              {validation.warnings.map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
          </div>
        ) : null}

        <div className="nt-form-grid">
          <label className="nt-field">
            <span>Profile 名称</span>
            <input
              className="nt-input"
              aria-invalid={Boolean(fieldErrors.name)}
              value={profile.name}
              onChange={(event) => updateText("name", event.currentTarget.value)}
            />
            {fieldErrors.name ? <small>{fieldErrors.name}</small> : null}
          </label>
          <fieldset className="nt-field nt-field--wide">
            <span>Runtime role</span>
            <div className="nt-segmented" role="group" aria-label="Gateway runtime role">
              {(["standalone", "splitter", "worker"] as const).map((role) => (
                <button
                  className={`nt-btn${profile.runtimeRole === role ? " nt-btn--primary" : " nt-btn--outline"}`}
                  type="button"
                  key={role}
                  aria-pressed={profile.runtimeRole === role}
                  onClick={() => patchProfile({ runtimeRole: role })}
                >
                  {role}
                </button>
              ))}
            </div>
            {fieldErrors.runtimeRole ? <small>{fieldErrors.runtimeRole}</small> : null}
          </fieldset>
          <label className="nt-field">
            <span>PORT</span>
            <input
              className="nt-input"
              aria-invalid={Boolean(fieldErrors.port)}
              min={1}
              max={65535}
              type="number"
              value={profile.port}
              onChange={(event) => patchProfile({ port: Number(event.currentTarget.value) })}
            />
            {fieldErrors.port ? <small>{fieldErrors.port}</small> : null}
          </label>
          <label className="nt-field nt-field--wide">
            <span>GATEWAY_MANAGEMENT_TOKEN</span>
            <input
              className="nt-input"
              type="password"
              autoComplete="off"
              value={optionalText(profile.gatewayManagementToken)}
              onChange={(event) => updateText("gatewayManagementToken", event.currentTarget.value)}
            />
            {fieldErrors.gatewayManagementToken ? (
              <small>{fieldErrors.gatewayManagementToken}</small>
            ) : null}
          </label>
          <label className="nt-field nt-field--wide">
            <span>GATEWAY_REDIS_URL</span>
            <input
              className="nt-input"
              aria-invalid={Boolean(fieldErrors.gatewayRedisUrl)}
              value={profile.gatewayRedisUrl}
              onChange={(event) => updateText("gatewayRedisUrl", event.currentTarget.value)}
            />
            {fieldErrors.gatewayRedisUrl ? <small>{fieldErrors.gatewayRedisUrl}</small> : null}
          </label>
          <label className="nt-field nt-field--wide">
            <span>GATEWAY_DATABASE_URL</span>
            <input
              className="nt-input"
              placeholder="可选：PostgreSQL / operator 数据库连接"
              value={optionalText(profile.gatewayDatabaseUrl)}
              aria-invalid={Boolean(fieldErrors.gatewayDatabaseUrl)}
              onChange={(event) => updateText("gatewayDatabaseUrl", event.currentTarget.value)}
            />
            {fieldErrors.gatewayDatabaseUrl ? <small>{fieldErrors.gatewayDatabaseUrl}</small> : null}
          </label>
          <label className="nt-field nt-field--wide">
            <span>GATEWAY_ROUTES_FILE</span>
            <input
              className="nt-input"
              placeholder="可选：routes.yaml 路径"
              value={optionalText(profile.gatewayRoutesFile)}
              onChange={(event) => updateText("gatewayRoutesFile", event.currentTarget.value)}
            />
          </label>
          <label className="nt-field">
            <span>RUST_LOG</span>
            <input
              className="nt-input"
              value={optionalText(profile.logLevel)}
              onChange={(event) => updateText("logLevel", event.currentTarget.value)}
            />
          </label>
          <label className="nt-field nt-field--wide">
            <span>Working directory</span>
            <input
              className="nt-input"
              placeholder="可选：默认使用 Gateway 子项目目录"
              value={optionalText(profile.workingDirectory)}
              onChange={(event) => updateText("workingDirectory", event.currentTarget.value)}
            />
          </label>
        </div>

        {state.pathCheck ? (
          <div className="nt-path-check" aria-label="Profile path check result">
            {renderPathCheckItem("Gateway sidecar", state.pathCheck.sidecar)}
            {renderPathCheckItem("Working directory", state.pathCheck.workingDirectory)}
            {renderPathCheckItem("GATEWAY_ROUTES_FILE", state.pathCheck.gatewayRoutesFile)}
            {renderDependencyCheck(state.pathCheck.redis)}
            {renderDependencyCheck(state.pathCheck.database)}
            {state.pathCheck.preflightMessages.length > 0 ? (
              <div className="nt-validation-list nt-validation-list--warning">
                <strong>Preflight</strong>
                <ul>
                  {state.pathCheck.preflightMessages.map((message) => (
                    <li key={message}>{message}</li>
                  ))}
                </ul>
              </div>
            ) : null}
          </div>
        ) : null}
      </article>

      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <h2>附加环境变量</h2>
          </div>
          <button
            className="nt-btn nt-btn--secondary"
            type="button"
            onClick={() => patchProfile({ extraEnv: [...profile.extraEnv, { key: "", value: "" }] })}
          >
            添加变量
          </button>
        </div>

        <div className="nt-env-list">
          {profile.extraEnv.length === 0 ? (
            <p className="nt-empty">暂无附加环境变量。核心 Gateway 配置仍由无 UI 运行时读取。</p>
          ) : (
            profile.extraEnv.map((entry, index) => {
              const envRowError = validation.extraEnvErrors[index];
              return (
                <div className="nt-env-row" key={`${entry.key}-${index}`}>
                  <div className="nt-env-row__key">
                    <input
                      className="nt-input"
                      placeholder="KEY"
                      aria-invalid={Boolean(envRowError)}
                      value={entry.key}
                      onChange={(event) =>
                        updateEnvEntry(index, { key: event.currentTarget.value })
                      }
                    />
                    {envRowError ? <small>{envRowError}</small> : null}
                  </div>
                  <input
                    className="nt-input"
                    placeholder="VALUE"
                    value={entry.value}
                    onChange={(event) =>
                      updateEnvEntry(index, { value: event.currentTarget.value })
                    }
                  />
                  <button
                    className="nt-btn nt-btn--outline"
                    type="button"
                    onClick={() =>
                      patchProfile({
                        extraEnv: profile.extraEnv.filter((_, entryIndex) => entryIndex !== index),
                      })
                    }
                  >
                    移除
                  </button>
                </div>
              );
            })
          )}
        </div>
      </article>

      <article className="nt-card nt-card--panel">
        <div className="nt-section__head">
          <div>
            <h2>Profile 导入/导出</h2>
          </div>
          <button
            className="nt-btn nt-btn--secondary"
            type="button"
            disabled={state.busy}
            onClick={() => void state.exportDraftProfile()}
          >
            导出脱敏 JSON
          </button>
        </div>
        <p className="nt-copy">
          导出内容会移除 URL 凭据和敏感环境变量值；导入只写入当前草稿，不会自动保存或启动
          Gateway。
        </p>
        {state.profileTransferText ? (
          <pre className="nt-code nt-transfer-box">{state.profileTransferText}</pre>
        ) : null}
        <label className="nt-field nt-field--wide">
          <span>粘贴 profile JSON</span>
          <textarea
            className="nt-input nt-textarea"
            placeholder="粘贴 gateway-ui-profile JSON，或直接粘贴 GatewayProfile JSON。"
            value={state.importProfileText}
            onChange={(event) => state.updateImportProfileText(event.currentTarget.value)}
          />
        </label>
        <div className="nt-actions">
          <button
            className="nt-btn nt-btn--primary"
            type="button"
            disabled={state.busy || state.importProfileText.trim().length === 0}
            onClick={() => void state.importDraftProfile()}
          >
            导入到草稿
          </button>
        </div>
      </article>
    </div>
  );
}
