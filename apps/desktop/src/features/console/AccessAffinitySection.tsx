import { Link2 } from "lucide-react";

import type {
  ConsoleAccessStickyAffinity,
  ConsoleApiAccessRotation,
} from "../../api/contracts";
import { Section } from "./AccessWorkspacePrimitives";
import type {
  AccessAffinityDraft,
  AccessPanelState,
  AccessSectionId,
  ApiAccessRotationDraft,
  TranslateFn,
} from "./accessKeysTypes";
import { formatText, formatTimestamp } from "./accessWorkspaceFormatting";

export type AccessAffinitySectionProps = {
  t: TranslateFn;
  editorLocked: boolean;
  affinityDraft: AccessAffinityDraft;
  onAffinityDraftChange: (next: AccessAffinityDraft) => void;
  onInspectAffinity: () => void;
  onResetAffinity: () => void;
  affinity: AccessPanelState<ConsoleAccessStickyAffinity | null>;
  affinityNotice: string | null;
  rotationDraft: ApiAccessRotationDraft;
  onRotationDraftChange: (next: ApiAccessRotationDraft) => void;
  onRotateApiAccess: () => void;
  rotatingApiAccess: boolean;
  lastRotation: ConsoleApiAccessRotation | null;
  affinityFormIncomplete: boolean;
  onToggle: (id: AccessSectionId) => void;
  open: boolean;
};

export function AccessAffinitySection({
  t,
  editorLocked,
  affinityDraft,
  onAffinityDraftChange,
  onInspectAffinity,
  onResetAffinity,
  affinity,
  affinityNotice,
  rotationDraft,
  onRotationDraftChange,
  onRotateApiAccess,
  rotatingApiAccess,
  lastRotation,
  affinityFormIncomplete,
  onToggle,
  open,
}: AccessAffinitySectionProps) {
  const patchAffinity = (patch: Partial<AccessAffinityDraft>) =>
    onAffinityDraftChange({ ...affinityDraft, ...patch });
  const patchRotation = (patch: Partial<ApiAccessRotationDraft>) =>
    onRotationDraftChange({ ...rotationDraft, ...patch });

  return (
    <Section
      icon={<Link2 size={17} />}
      id="affinity"
      onToggle={onToggle}
      open={open}
      title={t("会话粘性与项目密钥", "Affinity and project keys")}
    >
      <article className="nt-pilot-metric-card">
        <h3>{t("会话粘性", "Sticky affinity")}</h3>
        <p className="nt-pilot-stats-note">
          {t(
            "粘性记录只存放在 Redis，因此未配置 PostgreSQL 时依然可用。",
            "Affinity lives in Redis only, so it stays usable without PostgreSQL.",
          )}
        </p>
        <div className="nt-console-field-grid">
          <label className="nt-field">
            <span>{t("访问密钥 ID", "Access key id")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchAffinity({ accessKeyId: event.target.value })}
              value={affinityDraft.accessKeyId}
            />
          </label>
          <label className="nt-field">
            <span>{t("模型", "Model")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchAffinity({ model: event.target.value })}
              value={affinityDraft.model}
            />
          </label>
          <label className="nt-field">
            <span>{t("会话键", "Session key")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchAffinity({ explicitSessionKey: event.target.value })}
              placeholder={t("可选", "Optional")}
              value={affinityDraft.explicitSessionKey}
            />
          </label>
        </div>
        <div className="nt-console-form-actions">
          <button
            className="nt-btn nt-btn--primary"
            disabled={editorLocked || affinity.loading || affinityFormIncomplete}
            onClick={onInspectAffinity}
            type="button"
          >
            {affinity.loading ? t("查询中…", "Querying…") : t("查询粘性", "Inspect")}
          </button>
          <button
            className="nt-btn nt-btn--outline"
            disabled={editorLocked || affinity.loading || affinityFormIncomplete}
            onClick={onResetAffinity}
            type="button"
          >
            {t("清除粘性", "Reset")}
          </button>
        </div>
        {affinity.error ? (
          <div className="nt-alert nt-alert--warning" role="status">
            {affinity.error}
          </div>
        ) : null}
        {affinityNotice ? (
          <div className="nt-alert nt-alert--success" role="status">
            {affinityNotice}
          </div>
        ) : null}
        {affinity.data ? (
          <dl className="nt-pilot-metric-list">
            <div>
              <dt>{t("作用域", "Scope")}</dt>
              <dd>{formatText(affinity.data.scope)}</dd>
            </div>
            <div>
              <dt>{t("平台售卖行", "Platform access")}</dt>
              <dd>{formatText(affinity.data.platformAccessId)}</dd>
            </div>
            <div>
              <dt>{t("服务商账号", "Provider account")}</dt>
              <dd>{formatText(affinity.data.providerAccountId)}</dd>
            </div>
            <div>
              <dt>{t("来源密钥", "Source key")}</dt>
              <dd>{formatText(affinity.data.sourceAccessKeyId)}</dd>
            </div>
            <div>
              <dt>{t("过期时间", "Expires at")}</dt>
              <dd>{formatTimestamp(affinity.data.expiresAt)}</dd>
            </div>
          </dl>
        ) : (
          <div className="nt-pilot-stats-state" role="status">
            {t("尚无粘性记录", "No affinity recorded")}
          </div>
        )}
      </article>

      <article className="nt-pilot-metric-card">
        <h3>{t("轮换项目 API 密钥", "Rotate project API key")}</h3>
        <div className="nt-console-field-grid">
          <label className="nt-field">
            <span>{t("项目 ID", "Project id")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchRotation({ projectId: event.target.value })}
              value={rotationDraft.projectId}
            />
          </label>
          <label className="nt-field">
            <span>{t("密钥名称", "Key name")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchRotation({ name: event.target.value })}
              placeholder={t("可选", "Optional")}
              value={rotationDraft.name}
            />
          </label>
          <label className="nt-field">
            <span>{t("操作人 ID", "Actor user id")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchRotation({ actorUserId: event.target.value })}
              placeholder={t("可选", "Optional")}
              value={rotationDraft.actorUserId}
            />
          </label>
        </div>
        <div className="nt-console-form-actions">
          <button
            className="nt-btn nt-btn--primary"
            disabled={
              editorLocked || rotatingApiAccess || rotationDraft.projectId.trim().length === 0
            }
            onClick={onRotateApiAccess}
            type="button"
          >
            {rotatingApiAccess ? t("轮换中…", "Rotating…") : t("轮换", "Rotate")}
          </button>
        </div>
        {lastRotation ? (
          <dl className="nt-pilot-metric-list">
            <div>
              <dt>{t("项目", "Project")}</dt>
              <dd>{formatText(lastRotation.projectId)}</dd>
            </div>
            <div>
              <dt>{t("租户", "Tenant")}</dt>
              <dd>{formatText(lastRotation.tenantId)}</dd>
            </div>
          </dl>
        ) : null}
      </article>
    </Section>
  );
}
