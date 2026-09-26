import { ShieldCheck, UserRoundCheck } from "lucide-react";

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
  statusBadgeClass,
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
      <article className="nt-pilot-metric-card">
        <h3>{t("签发凭据", "Issue a credential")}</h3>
        <div className="nt-console-field-grid">
          <label className="nt-field">
            <span>{t("用户 ID", "User id")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchCredential({ userId: event.target.value })}
              value={credentialDraft.userId}
            />
          </label>
          <label className="nt-field">
            <span>{t("项目 ID", "Project id")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchCredential({ projectId: event.target.value })}
              placeholder={t("可选", "Optional")}
              value={credentialDraft.projectId}
            />
          </label>
          <label className="nt-field">
            <span>{t("凭据类型", "Credential type")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchCredential({ credentialType: event.target.value })}
              value={credentialDraft.credentialType}
            />
          </label>
          <label className="nt-field">
            <span>{t("有效天数", "Duration (days)")}</span>
            <input
              className="nt-input"
              inputMode="numeric"
              onChange={(event) => patchCredential({ durationDays: event.target.value })}
              value={credentialDraft.durationDays}
            />
          </label>
          <label className="nt-field nt-field--wide">
            <span>{t("授权范围", "Scopes")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchCredential({ scope: event.target.value })}
              placeholder="chat.completions, responses"
              value={credentialDraft.scope}
            />
          </label>
        </div>
        <div className="nt-console-form-actions">
          <button
            className="nt-btn nt-btn--primary"
            disabled={
              editorLocked || issuingCredential || credentialDraft.userId.trim().length === 0
            }
            onClick={onIssueCredential}
            type="button"
          >
            {issuingCredential ? t("签发中…", "Issuing…") : t("签发", "Issue")}
          </button>
          <button
            className="nt-btn nt-btn--secondary"
            disabled={editorLocked}
            onClick={() => onCredentialDraftChange({ ...ACCESS_DEFAULT_CREDENTIAL_DRAFT })}
            type="button"
          >
            {t("重置", "Reset")}
          </button>
        </div>
        {lastIssuedCredential ? (
          <dl className="nt-pilot-metric-list">
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
      </article>

      <article className="nt-pilot-metric-card">
        <h3>{t("校验与吊销", "Verify and revoke")}</h3>
        <div className="nt-console-field-grid">
          <label className="nt-field nt-field--wide">
            <span>{t("凭据明文", "Credential key")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchVerify({ credentialKey: event.target.value })}
              value={verifyDraft.credentialKey}
            />
          </label>
          <label className="nt-field">
            <span>{t("校验范围", "Scope to check")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchVerify({ scope: event.target.value })}
              placeholder={t("可选，单个范围", "Optional, single scope")}
              value={verifyDraft.scope}
            />
          </label>
        </div>
        <div className="nt-console-form-actions">
          <button
            className="nt-btn nt-btn--primary"
            disabled={
              editorLocked || credentialBusy || verifyDraft.credentialKey.trim().length === 0
            }
            onClick={onVerifyCredential}
            type="button"
          >
            {credentialBusy ? t("处理中…", "Working…") : t("校验", "Verify")}
          </button>
          <button
            className="nt-btn nt-btn--outline"
            disabled={
              editorLocked || credentialBusy || verifyDraft.credentialKey.trim().length === 0
            }
            onClick={onRevokeCredential}
            type="button"
          >
            {t("吊销", "Revoke")}
          </button>
        </div>
        {verification.error ? (
          <div className="nt-alert nt-alert--warning" role="status">
            {verification.error}
          </div>
        ) : null}
        {credentialNotice ? (
          <div className="nt-alert nt-alert--success" role="status">
            {credentialNotice}
          </div>
        ) : null}
        {verification.data ? (
          <>
            <div className="nt-console-chip-row">
              <span
                className={
                  verification.data.valid
                    ? "nt-badge nt-badge--success"
                    : "nt-badge nt-badge--danger"
                }
              >
                <ShieldCheck size={13} aria-hidden="true" />
                {verification.data.valid ? t("有效", "Valid") : t("无效", "Invalid")}
              </span>
              <span className="nt-chip nt-chip--muted">
                {formatText(verification.data.reason)}
              </span>
            </div>
            {verification.data.credential ? (
              <dl className="nt-pilot-metric-list">
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
                    <span className={statusBadgeClass(verification.data.credential.status)}>
                      {formatText(verification.data.credential.status)}
                    </span>
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
      </article>
    </Section>
  );
}
