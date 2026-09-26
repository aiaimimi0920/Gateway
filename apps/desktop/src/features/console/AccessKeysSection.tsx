import { KeyRound } from "lucide-react";

import type { ConsoleAccessBundle, ConsoleAccessCatalog, ConsoleAccessKey } from "../../api/contracts";
import { CopyButton, Section, StatCard } from "./AccessWorkspacePrimitives";
import {
  ACCESS_DEFAULT_KEY_DRAFT,
  type AccessKeyDraft,
  type AccessPanelState,
  type AccessSectionId,
  type TranslateFn,
} from "./accessKeysTypes";
import {
  formatCount,
  formatText,
  formatTimestamp,
  maskSecret,
  PLACEHOLDER,
  statusBadgeClass,
} from "./accessWorkspaceFormatting";

export type AccessKeysSectionProps = {
  t: TranslateFn;
  editorLocked: boolean;
  catalog: AccessPanelState<ConsoleAccessCatalog>;
  keyDraft: AccessKeyDraft;
  onKeyDraftChange: (next: AccessKeyDraft) => void;
  onCreateKey: () => void;
  creatingKey: boolean;
  keyBusyId: string | null;
  onRotateKey: (accessKeyId: string) => void;
  onRevokeKey: (accessKeyId: string) => void;
  bundles: ConsoleAccessBundle[];
  sortedKeys: ConsoleAccessKey[];
  bundlesByKey: Map<string, string[]>;
  activeKeyCount: number;
  catalogEmpty: boolean;
  keyFormIncomplete: boolean;
  onToggle: (id: AccessSectionId) => void;
  open: boolean;
};

export function AccessKeysSection({
  t,
  editorLocked,
  catalog,
  keyDraft,
  onKeyDraftChange,
  onCreateKey,
  creatingKey,
  keyBusyId,
  onRotateKey,
  onRevokeKey,
  bundles,
  sortedKeys,
  bundlesByKey,
  activeKeyCount,
  catalogEmpty,
  keyFormIncomplete,
  onToggle,
  open,
}: AccessKeysSectionProps) {
  const patchKey = (patch: Partial<AccessKeyDraft>) =>
    onKeyDraftChange({ ...keyDraft, ...patch });
  const toggleDraftBundle = (bundleId: string) => {
    const selected = keyDraft.bundleIds.includes(bundleId);
    patchKey({
      bundleIds: selected
        ? keyDraft.bundleIds.filter((entry) => entry !== bundleId)
        : [...keyDraft.bundleIds, bundleId],
    });
  };

  return (
    <Section
      icon={<KeyRound size={17} />}
      id="keys"
      onToggle={onToggle}
      open={open}
      title={t("访问密钥", "Access keys")}
    >
      {catalog.error ? (
        <div className="nt-alert nt-alert--warning" role="status">
          {catalog.error}
        </div>
      ) : null}

      <div className="nt-pilot-stats-overview-grid">
        <StatCard
          label={t("生效密钥", "Active keys")}
          tone="emerald"
          value={catalogEmpty ? PLACEHOLDER : formatCount(activeKeyCount)}
        />
        <StatCard
          label={t("已吊销", "Revoked")}
          tone="danger"
          value={catalogEmpty ? PLACEHOLDER : formatCount(sortedKeys.length - activeKeyCount)}
        />
        <StatCard
          label={t("权益包", "Bundles")}
          tone="blue"
          value={catalogEmpty ? PLACEHOLDER : formatCount(bundles.length)}
        />
        <StatCard
          label={t("平台售卖行", "Platform access rows")}
          value={
            catalogEmpty ? PLACEHOLDER : formatCount(catalog.data?.platformAccessRows.length)
          }
        />
      </div>

      <article className="nt-pilot-metric-card">
        <h3>{t("签发新密钥", "Issue a key")}</h3>
        <div className="nt-console-field-grid">
          <label className="nt-field">
            <span>{t("归属类型", "Owner type")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchKey({ ownerType: event.target.value })}
              placeholder="user / tenant"
              value={keyDraft.ownerType}
            />
          </label>
          <label className="nt-field">
            <span>{t("归属 ID", "Owner id")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchKey({ ownerId: event.target.value })}
              value={keyDraft.ownerId}
            />
          </label>
          <label className="nt-field">
            <span>{t("项目 ID", "Project id")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchKey({ resolvedProjectId: event.target.value })}
              value={keyDraft.resolvedProjectId}
            />
          </label>
          <label className="nt-field">
            <span>{t("租户 ID", "Tenant id")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchKey({ resolvedTenantId: event.target.value })}
              value={keyDraft.resolvedTenantId}
            />
          </label>
          <label className="nt-field">
            <span>{t("密钥类型", "Key kind")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchKey({ keyKind: event.target.value })}
              placeholder="user / aggregate"
              value={keyDraft.keyKind}
            />
          </label>
          <label className="nt-field">
            <span>{t("前缀", "Public prefix")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchKey({ publicKeyPrefix: event.target.value })}
              value={keyDraft.publicKeyPrefix}
            />
          </label>
          <label className="nt-field">
            <span>{t("显示名", "Display name")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchKey({ displayName: event.target.value })}
              value={keyDraft.displayName}
            />
          </label>
          <label className="nt-field">
            <span>{t("过期时间", "Expires at")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchKey({ expiresAt: event.target.value })}
              placeholder="2026-12-31T00:00:00Z"
              value={keyDraft.expiresAt}
            />
          </label>
        </div>

        {bundles.length > 0 ? (
          <fieldset className="nt-field nt-field--wide">
            <span>{t("绑定权益包", "Bind bundles")}</span>
            <div className="nt-console-chip-row">
              {bundles.map((bundle) => {
                const selected = keyDraft.bundleIds.includes(bundle.id);
                return (
                  <button
                    aria-pressed={selected}
                    className={`nt-btn ${selected ? "nt-btn--primary" : "nt-btn--secondary"}`}
                    disabled={editorLocked}
                    key={bundle.id}
                    onClick={() => toggleDraftBundle(bundle.id)}
                    type="button"
                  >
                    {bundle.displayName || bundle.slug}
                  </button>
                );
              })}
            </div>
          </fieldset>
        ) : null}

        <div className="nt-console-form-actions">
          <button
            className="nt-btn nt-btn--primary"
            disabled={editorLocked || creatingKey || keyFormIncomplete}
            onClick={onCreateKey}
            type="button"
          >
            {creatingKey ? t("签发中…", "Issuing…") : t("签发密钥", "Issue key")}
          </button>
          <button
            className="nt-btn nt-btn--secondary"
            disabled={editorLocked}
            onClick={() => onKeyDraftChange({ ...ACCESS_DEFAULT_KEY_DRAFT })}
            type="button"
          >
            {t("重置", "Reset")}
          </button>
        </div>
      </article>

      <article className="nt-pilot-metric-card">
        <h3>{t("密钥台账", "Key ledger")}</h3>
        {catalog.loading && catalogEmpty ? (
          <div className="nt-pilot-stats-state" role="status">
            {t("正在读取…", "Loading…")}
          </div>
        ) : sortedKeys.length === 0 ? (
          <div className="nt-pilot-stats-state" role="status">
            {t("暂无访问密钥", "No access keys yet")}
          </div>
        ) : (
          <div className="nt-table nt-table--access-keys">
            <div className="nt-table__head">
              <span>{t("名称", "Name")}</span>
              <span>{t("状态", "Status")}</span>
              <span>{t("权益包", "Bundles")}</span>
              <span>{t("最近使用", "Last used")}</span>
              <span>{t("操作", "Actions")}</span>
            </div>
            {sortedKeys.map((key) => {
              const busy = keyBusyId === key.id;
              const revoked = Boolean(key.revokedAt);
              const boundBundles = bundlesByKey.get(key.id) ?? [];
              return (
                <div className="nt-table__row" key={key.id}>
                  <span title={key.id}>
                    {formatText(key.displayName)}
                    <small className="nt-console-subtext">
                      {key.publicKeyPrefix} · {maskSecret(key.id)}
                    </small>
                  </span>
                  <span>
                    <span className={statusBadgeClass(revoked ? "revoked" : key.status)}>
                      {revoked ? t("已吊销", "Revoked") : formatText(key.status)}
                    </span>
                  </span>
                  <span title={boundBundles.join(", ")}>
                    {boundBundles.length > 0 ? boundBundles.join(", ") : PLACEHOLDER}
                  </span>
                  <span>{formatTimestamp(key.lastUsedAt)}</span>
                  <span className="nt-console-row-actions">
                    <CopyButton
                      label={t("复制 ID", "Copy ID")}
                      t={t}
                      value={key.id}
                      variant="outline"
                    />
                    <button
                      className="nt-btn nt-btn--secondary"
                      disabled={editorLocked || busy || revoked}
                      onClick={() => onRotateKey(key.id)}
                      type="button"
                    >
                      {t("轮换", "Rotate")}
                    </button>
                    <button
                      className="nt-btn nt-btn--outline"
                      disabled={editorLocked || busy || revoked}
                      onClick={() => onRevokeKey(key.id)}
                      type="button"
                    >
                      {t("吊销", "Revoke")}
                    </button>
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </article>
    </Section>
  );
}
