import { UserRoundCheck } from "lucide-react";
import { Badge, Button, Field, Input, Notice, Panel } from "@neuro/ui";

import {
  ACCESS_DEFAULT_CREDENTIAL_DRAFT,
  type AccessKeysWorkspaceProps,
  type UserCredentialDraft,
  type UserCredentialVerifyDraft,
} from "./accessKeysTypes";
import { Section } from "./AccessWorkspacePrimitives";
import {
  formatText,
  formatTimestamp,
  PLACEHOLDER,
  accessStatusTone,
} from "./accessWorkspaceFormatting";

type AccessCredentialsSectionProps = Pick<
  AccessKeysWorkspaceProps,
  | "t"
  | "editorLocked"
  | "credentialDraft"
  | "onCredentialDraftChange"
  | "onIssueCredential"
  | "issuingCredential"
  | "lastIssuedCredential"
  | "verifyDraft"
  | "onVerifyDraftChange"
  | "onVerifyCredential"
  | "onRevokeCredential"
  | "credentialBusy"
  | "verification"
  | "credentialNotice"
> & {
  onToggle: () => void;
  open: boolean;
};

/** Owns the user-credential issue, verify, and revoke presentation only. */
export function AccessCredentialsSection({
  t,
  editorLocked,
  credentialDraft,
  onCredentialDraftChange,
  onIssueCredential,
  issuingCredential,
  lastIssuedCredential,
  verifyDraft,
  onVerifyDraftChange,
  onVerifyCredential,
  onRevokeCredential,
  credentialBusy,
  verification,
  credentialNotice,
  onToggle,
  open,
}: AccessCredentialsSectionProps) {
  const patchCredential = (patch: Partial<UserCredentialDraft>) =>
    onCredentialDraftChange({ ...credentialDraft, ...patch });
  const patchVerify = (patch: Partial<UserCredentialVerifyDraft>) =>
    onVerifyDraftChange({ ...verifyDraft, ...patch });

  return (
    <Section
      icon={<UserRoundCheck size={17} />}
      id="credentials"
      onToggle={onToggle}
      open={open}
      title={t("用户凭据", "User credentials")}
    >
      <Panel className="nt-access-card">
        <h3>{t("签发凭据", "Issue a credential")}</h3>
        <div className="nt-access-fields">
          <Field label={t("用户 ID", "User id")}>
            <Input
              onChange={(event) => patchCredential({ userId: event.target.value })}
              value={credentialDraft.userId}
            />
          </Field>
          <Field label={t("项目 ID", "Project id")}>
            <Input
              onChange={(event) => patchCredential({ projectId: event.target.value })}
              placeholder={t("可选", "Optional")}
              value={credentialDraft.projectId}
            />
          </Field>
          <Field label={t("凭据类型", "Credential type")}>
            <Input
              onChange={(event) => patchCredential({ credentialType: event.target.value })}
              value={credentialDraft.credentialType}
            />
          </Field>
          <Field label={t("有效天数", "Duration (days)")}>
            <Input
              inputMode="numeric"
              onChange={(event) => patchCredential({ durationDays: event.target.value })}
              value={credentialDraft.durationDays}
            />
          </Field>
          <div className="nt-access-field--wide"><Field label={t("授权范围", "Scopes")}>
            <Input
              onChange={(event) => patchCredential({ scope: event.target.value })}
              placeholder="chat.completions, responses"
              value={credentialDraft.scope}
            />
          </Field></div>
        </div>
        <div className="nt-access-actions">
          <Button variant="primary" loading={issuingCredential}
            disabled={
              editorLocked || issuingCredential || credentialDraft.userId.trim().length === 0
            }
            onClick={onIssueCredential}
            type="button"
          >
            {t("签发", "Issue")}
          </Button>
          <Button
            disabled={editorLocked}
            onClick={() => onCredentialDraftChange({ ...ACCESS_DEFAULT_CREDENTIAL_DRAFT })}
            type="button"
          >
            {t("重置", "Reset")}
          </Button>
        </div>
        {lastIssuedCredential ? (
          <dl className="nt-access-metrics">
            <div>
              <dt>{t("凭据 ID", "Credential id")}</dt>
              <dd>{formatText(lastIssuedCredential.id)}</dd>
            </div>
            <div>
              <dt>{t("用户", "User")}</dt>
              <dd>{formatText(lastIssuedCredential.userId)}</dd>
            </div>
            <div>
              <dt>{t("过期时间", "Expires at")}</dt>
              <dd>{formatTimestamp(lastIssuedCredential.expiresAt)}</dd>
            </div>
            <div>
              <dt>{t("范围", "Scopes")}</dt>
              <dd>
                {lastIssuedCredential.scope.length > 0
                  ? lastIssuedCredential.scope.join(", ")
                  : PLACEHOLDER}
              </dd>
            </div>
          </dl>
        ) : null}
      </Panel>

      <Panel className="nt-access-card">
        <h3>{t("校验与吊销", "Verify and revoke")}</h3>
        <div className="nt-access-fields">
          <div className="nt-access-field--wide"><Field label={t("凭据明文", "Credential key")}>
            <Input
              onChange={(event) => patchVerify({ credentialKey: event.target.value })}
              value={verifyDraft.credentialKey}
            />
          </Field></div>
          <Field label={t("校验范围", "Scope to check")}>
            <Input
              onChange={(event) => patchVerify({ scope: event.target.value })}
              placeholder={t("可选，单个范围", "Optional, single scope")}
              value={verifyDraft.scope}
            />
          </Field>
        </div>
        <div className="nt-access-actions">
          <Button variant="primary" loading={credentialBusy}
            disabled={
              editorLocked || credentialBusy || verifyDraft.credentialKey.trim().length === 0
            }
            onClick={onVerifyCredential}
            type="button"
          >
            {t("校验", "Verify")}
          </Button>
          <Button
            disabled={
              editorLocked || credentialBusy || verifyDraft.credentialKey.trim().length === 0
            }
            onClick={onRevokeCredential}
            type="button"
          >
            {t("吊销", "Revoke")}
          </Button>
        </div>
        {verification.error ? (
          <Notice tone="warning">
            {verification.error}
          </Notice>
        ) : null}
        {credentialNotice ? (
          <Notice tone="success">
            {credentialNotice}
          </Notice>
        ) : null}
        {verification.data ? (
          <>
            <div className="nt-access-result">
              <Badge tone={verification.data.valid ? "success" : "danger"}>
                {verification.data.valid ? t("有效", "Valid") : t("无效", "Invalid")}
              </Badge>
              <span>
                {formatText(verification.data.reason)}
              </span>
            </div>
            {verification.data.credential ? (
              <dl className="nt-access-metrics">
                <div>
                  <dt>{t("用户", "User")}</dt>
                  <dd>{formatText(verification.data.credential.userId)}</dd>
                </div>
                <div>
                  <dt>{t("项目", "Project")}</dt>
                  <dd>{formatText(verification.data.credential.projectId)}</dd>
                </div>
                <div>
                  <dt>{t("状态", "Status")}</dt>
                  <dd>
                    <Badge tone={accessStatusTone(verification.data.credential.status)}>
                      {formatText(verification.data.credential.status)}
                    </Badge>
                  </dd>
                </div>
                <div>
                  <dt>{t("过期时间", "Expires at")}</dt>
                  <dd>{formatTimestamp(verification.data.credential.expiresAt)}</dd>
                </div>
                <div>
                  <dt>{t("范围", "Scopes")}</dt>
                  <dd>
                    {verification.data.credential.scope.length > 0
                      ? verification.data.credential.scope.join(", ")
                      : PLACEHOLDER}
                  </dd>
                </div>
              </dl>
            ) : null}
          </>
        ) : null}
      </Panel>
    </Section>
  );
}
