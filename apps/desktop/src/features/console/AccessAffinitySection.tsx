import { Link2 } from "lucide-react";
import { Button, EmptyState, Field, Input, Notice, Panel } from "@neuro/ui";

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
      <Panel className="nt-access-card">
        <h3>{t("会话粘性", "Sticky affinity")}</h3>
        <div className="nt-access-fields">
          <Field label={t("访问密钥 ID", "Access key id")}>
            <Input
              onChange={(event) => patchAffinity({ accessKeyId: event.target.value })}
              value={affinityDraft.accessKeyId}
            />
          </Field>
          <Field label={t("模型", "Model")}>
            <Input
              onChange={(event) => patchAffinity({ model: event.target.value })}
              value={affinityDraft.model}
            />
          </Field>
          <Field label={t("会话键", "Session key")}>
            <Input
              onChange={(event) => patchAffinity({ explicitSessionKey: event.target.value })}
              placeholder={t("可选", "Optional")}
              value={affinityDraft.explicitSessionKey}
            />
          </Field>
        </div>
        <div className="nt-access-actions">
          <Button variant="primary" loading={affinity.loading}
            disabled={editorLocked || affinity.loading || affinityFormIncomplete}
            onClick={onInspectAffinity}
            type="button"
          >
            {t("查询粘性", "Inspect")}
          </Button>
          <Button
            disabled={editorLocked || affinity.loading || affinityFormIncomplete}
            onClick={onResetAffinity}
            type="button"
          >
            {t("清除粘性", "Reset")}
          </Button>
        </div>
        {affinity.error ? (
          <Notice tone="warning">
            {affinity.error}
          </Notice>
        ) : null}
        {affinityNotice ? (
          <Notice tone="success">
            {affinityNotice}
          </Notice>
        ) : null}
        {affinity.data ? (
          <dl className="nt-access-metrics">
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
          <div role="status">
            <EmptyState title={t("尚无粘性记录", "No affinity recorded")} />
          </div>
        )}
      </Panel>

      <Panel className="nt-access-card">
        <h3>{t("轮换项目 API 密钥", "Rotate project API key")}</h3>
        <div className="nt-access-fields">
          <Field label={t("项目 ID", "Project id")}>
            <Input
              onChange={(event) => patchRotation({ projectId: event.target.value })}
              value={rotationDraft.projectId}
            />
          </Field>
          <Field label={t("密钥名称", "Key name")}>
            <Input
              onChange={(event) => patchRotation({ name: event.target.value })}
              placeholder={t("可选", "Optional")}
              value={rotationDraft.name}
            />
          </Field>
          <Field label={t("操作人 ID", "Actor user id")}>
            <Input
              onChange={(event) => patchRotation({ actorUserId: event.target.value })}
              placeholder={t("可选", "Optional")}
              value={rotationDraft.actorUserId}
            />
          </Field>
        </div>
        <div className="nt-access-actions">
          <Button variant="primary" loading={rotatingApiAccess}
            disabled={
              editorLocked || rotatingApiAccess || rotationDraft.projectId.trim().length === 0
            }
            onClick={onRotateApiAccess}
            type="button"
          >
            {t("轮换", "Rotate")}
          </Button>
        </div>
        {lastRotation ? (
          <dl className="nt-access-metrics">
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
      </Panel>
    </Section>
  );
}
