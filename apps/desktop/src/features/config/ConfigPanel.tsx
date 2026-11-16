import { Badge, Button, Field, Notice, Panel, Select, Textarea } from "@neuro/ui";
import type { GatewayDesktopState, GatewayProfile } from "../../lib/types";
import { PROFILE_TEMPLATES } from "../../lib/profileTemplates";
import { ProfileFields } from "./ProfileFields";
import { ProfileEnvironmentFields } from "./ProfileEnvironmentFields";
import { ProfilePathResults } from "./ProfilePathResults";

type ConfigPanelProps = {
  state: GatewayDesktopState;
};

export function ConfigPanel({ state }: ConfigPanelProps) {
  const profile = state.draftProfile;
  const browserPreviewMode = !state.isTauriAvailable;
  const validation = state.profileValidation;

  const patchProfile = (patch: Partial<GatewayProfile>) => {
    state.updateDraftProfile({ ...profile, ...patch });
  };

  return (
    <div className="gateway-desktop-panels">
      <Panel className="gateway-desktop-panel">
        <div className="nt-section__head">
          <div>
            <h2>Profile 快速模板</h2>
          </div>
        </div>
        <Notice>模板只更新草稿，不会保存或启动 Gateway。</Notice>
        <div className="nt-template-grid">
          {PROFILE_TEMPLATES.map((template) => (
            <Button
              className="gateway-profile-template"
              key={template.id}
              type="button"
              disabled={state.busy}
              onClick={() => state.applyProfileTemplate(template.id)}
            >
              <span>{template.title}</span>
              <small>{template.description}</small>
            </Button>
          ))}
        </div>
      </Panel>

      <Panel className="gateway-desktop-panel">
        <div className="nt-section__head">
          <div>
            <h2>本地运行配置</h2>
            {state.hasUnsavedProfileChanges ? (
              <Badge tone="warning">未保存变更</Badge>
            ) : null}
          </div>
          <div className="nt-actions">
            <Select aria-label="已保存的 Profile"
              value={state.selectedProfileName}
              disabled={!state.isTauriAvailable}
              onChange={(event) => void state.selectProfile(event.currentTarget.value)}
            >
              {state.profileNames.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </Select>
            <Button
              variant="primary"
              type="button"
              disabled={state.busy || !state.canSaveProfile}
              onClick={() => void state.saveDraftProfile()}
            >
              保存 profile
            </Button>
            <Button
              variant="quiet"
              type="button"
              disabled={state.busy || !state.isTauriAvailable}
              onClick={() => void state.deleteSelectedProfile()}
            >
              删除/重置
            </Button>
            <Button
              variant="secondary"
              type="button"
              disabled={state.busy || !state.isTauriAvailable}
              onClick={() => void state.checkProfilePaths()}
            >
              检查路径
            </Button>
          </div>
        </div>

        {browserPreviewMode ? (
          <Notice tone="warning">浏览器预览不能读取或持久化本地 profile。</Notice>
        ) : null}

        {state.profileValidation.errors.length > 0 ? (
          <Notice tone="danger">
            <strong>配置需要修正</strong>
            <ul>
              {validation.errors.map((error) => (
                <li key={error}>{error}</li>
              ))}
            </ul>
          </Notice>
        ) : validation.warnings.length > 0 ? (
          <Notice tone="warning">
            <strong>保存前后端会继续检查</strong>
            <ul>
              {validation.warnings.map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
          </Notice>
        ) : null}

        <ProfileFields profile={profile} errors={validation.fieldErrors} onChange={patchProfile} />

        {state.pathCheck ? <ProfilePathResults result={state.pathCheck} /> : null}
      </Panel>

      <Panel className="gateway-desktop-panel">
        <ProfileEnvironmentFields entries={profile.extraEnv} errors={validation.extraEnvErrors}
          onChange={extraEnv => patchProfile({ extraEnv })} />
      </Panel>

      <Panel className="gateway-desktop-panel">
        <div className="nt-section__head">
          <div>
            <h2>Profile 导入/导出</h2>
          </div>
          <Button
            variant="secondary"
            type="button"
            disabled={state.busy}
            onClick={() => void state.exportDraftProfile()}
          >
            导出脱敏 JSON
          </Button>
        </div>
        <Notice>
          导出内容会移除 URL 凭据和敏感环境变量值；导入只写入当前草稿，不会自动保存或启动
          Gateway。
        </Notice>
        {state.profileTransferText ? (
          <pre className="nt-code nt-transfer-box">{state.profileTransferText}</pre>
        ) : null}
        <Field label="粘贴 profile JSON">
          <Textarea
            placeholder="粘贴 gateway-ui-profile JSON，或直接粘贴 GatewayProfile JSON。"
            value={state.importProfileText}
            onChange={(event) => state.updateImportProfileText(event.currentTarget.value)}
          />
        </Field>
        <div className="nt-actions">
          <Button
            variant="primary"
            type="button"
            disabled={state.busy || state.importProfileText.trim().length === 0}
            onClick={() => void state.importDraftProfile()}
          >
            导入到草稿
          </Button>
        </div>
      </Panel>
    </div>
  );
}
