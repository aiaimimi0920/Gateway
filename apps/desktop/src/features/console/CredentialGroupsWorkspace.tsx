import { Plus, UserMinus, UserPlus } from "lucide-react";
import type { ReactNode } from "react";

import { ActionTooltip } from "../../components/ActionTooltip";
import type {
  AccountGroupDraftLike,
  CredentialGroupDirectoryItem,
  GroupMemberCandidate,
} from "./accountManagementViewModel";

type TranslateFn = (zh: string, en: string) => string;

export type CredentialGroupsWorkspaceProps = {
  t: TranslateFn;
  notice?: ReactNode;
  editorLocked: boolean;
  totalAccounts: number;
  totalGroups: number;
  enabledGroupCount: number;
  ungroupedCount: number;
  groups: CredentialGroupDirectoryItem[];
  selectedGroupRowId: string | null;
  selectedGroup: AccountGroupDraftLike | null;
  selectedGroupIdInvalid: boolean;
  selectedGroupBillingInvalid: boolean;
  memberCandidates: GroupMemberCandidate[];
  memberQuery: string;
  memberMode: "all" | "members" | "ungrouped";
  onSelectGroup: (rowId: string) => void;
  onAddGroup: () => void;
  onBackToAccounts: () => void;
  onUpdateField: (
    rowId: string,
    field: "groupId" | "name" | "description" | "billingMultiplier" | "notes",
    value: string,
  ) => void;
  onToggleEnabled: (rowId: string, enabled: boolean) => void;
  onRemoveGroup: (rowId: string) => void;
  onMemberQueryChange: (value: string) => void;
  onMemberModeChange: (value: "all" | "members" | "ungrouped") => void;
  onToggleMember: (rowId: string, accountId: string) => void;
};

function selectedGroupTitle(t: TranslateFn, group: AccountGroupDraftLike | null): string {
  if (!group) {
    return t("未选择分组", "No group selected");
  }
  return group.name.trim() || group.groupId.trim() || t("新分组", "New group");
}

export function CredentialGroupsWorkspace({
  t,
  notice,
  editorLocked,
  totalAccounts,
  totalGroups,
  enabledGroupCount,
  ungroupedCount,
  groups,
  selectedGroupRowId,
  selectedGroup,
  selectedGroupIdInvalid,
  selectedGroupBillingInvalid,
  memberCandidates,
  memberQuery,
  memberMode,
  onSelectGroup,
  onAddGroup,
  onBackToAccounts,
  onUpdateField,
  onToggleEnabled,
  onRemoveGroup,
  onMemberQueryChange,
  onMemberModeChange,
  onToggleMember,
}: CredentialGroupsWorkspaceProps) {
  const groupTitle = selectedGroupTitle(t, selectedGroup);
  const selectedDirectoryItem =
    groups.find((group) => group.rowId === selectedGroupRowId) ?? null;
  const groupIdErrorId = selectedGroup ? `${selectedGroup.id}-group-id-error` : undefined;
  const billingErrorId = selectedGroup ? `${selectedGroup.id}-billing-multiplier-error` : undefined;

  return (
    <div className="nt-stack">
      {notice}
      <article className="nt-card nt-card--panel nt-groups-shell">
        <div className="nt-section__head">
          <div>
            <p className="nt-kicker">// Credential Groups</p>
            <h2>{t("凭证分组", "Credential Groups")}</h2>
            <p className="nt-copy">
              {t(
                "将不同账号组织进可复用凭证池，并维护路由可消费的分组元数据。",
                "Organize accounts into reusable credential pools and maintain routing metadata for each group.",
              )}
            </p>
          </div>
          <div className="nt-actions">
            <button className="nt-btn nt-btn--secondary" type="button" disabled={editorLocked} onClick={onAddGroup}>
              <Plus size={15} aria-hidden="true" />
              {t("添加分组", "Add group")}
            </button>
            <button className="nt-btn nt-btn--outline" type="button" onClick={onBackToAccounts}>
              {t("返回账号台账", "Back to accounts")}
            </button>
          </div>
        </div>

        <div className="nt-ledger-stats">
          <article className="nt-card nt-card--stat nt-ledger-stat">
            <span>{t("已定义分组", "Groups")}</span>
            <strong>{totalGroups}</strong>
          </article>
          <article className="nt-card nt-card--stat nt-ledger-stat">
            <span>{t("已启用分组", "Enabled groups")}</span>
            <strong>{enabledGroupCount}</strong>
          </article>
          <article className="nt-card nt-card--stat nt-ledger-stat">
            <span>{t("未分组账号", "Ungrouped accounts")}</span>
            <strong>{ungroupedCount}</strong>
          </article>
          <article className="nt-card nt-card--stat nt-ledger-stat">
            <span>{t("可选账号", "Accounts available")}</span>
            <strong>{totalAccounts}</strong>
          </article>
        </div>
      </article>

      {groups.length === 0 ? (
        <article className="nt-card nt-card--panel nt-empty-state">
          <h2>{t("暂无分组", "No groups yet")}</h2>
          <p className="nt-copy">
            {t(
              "当前还没有任何凭证分组。先创建一个分组，再把不同账号组织进可复用池。",
              "No credential groups exist yet. Create one first, then organize accounts into reusable pools.",
            )}
          </p>
          <div className="nt-actions">
            <button className="nt-btn nt-btn--secondary" type="button" disabled={editorLocked} onClick={onAddGroup}>
              <Plus size={15} aria-hidden="true" />
              {t("创建第一个分组", "Create first group")}
            </button>
          </div>
        </article>
      ) : (
        <section className="nt-group-admin" aria-label={t("凭证分组管理", "Credential group administration")}>
          <nav className="nt-group-admin__list" aria-label={t("分组列表", "Group list")}>
            {groups.map((group) => {
              const active = group.rowId === selectedGroupRowId;
              return (
                <button
                  key={group.rowId}
                  type="button"
                  className={`nt-group-directory-item${active ? " nt-group-directory-item--active" : ""}`}
                  aria-pressed={active}
                  onClick={() => onSelectGroup(group.rowId)}
                >
                  <strong>{group.name || group.groupId || t("新分组", "New group")}</strong>
                  <span>{group.groupId || t("未填写 ID", "Missing ID")}</span>
                  <div className="nt-ledger-chip-list">
                    <span className={group.enabled ? "nt-badge nt-badge--success" : "nt-badge nt-badge--warning"}>
                      {group.enabled ? t("启用", "Enabled") : t("停用", "Disabled")}
                    </span>
                    <span className="nt-chip">
                      {t("成员", "Members")} {group.memberCount}
                    </span>
                  </div>
                  <small>
                    {group.providerLabels.length > 0
                      ? group.providerLabels.join(", ")
                      : t("暂无成员", "No members yet")}
                  </small>
                </button>
              );
            })}
          </nav>

          <section className="nt-group-admin__detail" aria-label={t("分组详情", "Group detail")}>
            {selectedGroup ? (
              <>
                <article className="nt-card nt-card--panel nt-group-panel nt-group-panel--editor">
                  <div className="nt-section__head">
                    <div>
                      <p className="nt-kicker">// {selectedGroup.groupId || groupTitle}</p>
                      <h3>{groupTitle}</h3>
                    </div>
                    <div className="nt-actions">
                      <label className="nt-chip">
                        <input
                          type="checkbox"
                          checked={selectedGroup.enabled}
                          disabled={editorLocked}
                          aria-label={t(`${groupTitle} 启用状态`, `${groupTitle} enabled state`)}
                          onChange={(event) => onToggleEnabled(selectedGroup.id, event.currentTarget.checked)}
                        />
                        <span>{selectedGroup.enabled ? t("启用", "Enabled") : t("停用", "Disabled")}</span>
                      </label>
                      <button
                        className="nt-btn nt-btn--outline"
                        type="button"
                        disabled={editorLocked}
                        onClick={() => onRemoveGroup(selectedGroup.id)}
                      >
                        {t("移除分组", "Remove group")}
                      </button>
                    </div>
                  </div>

                  <div className="nt-form-grid">
                    <label className="nt-field">
                      <span>{t("分组 ID", "Group ID")}</span>
                      <input
                        className="nt-input"
                        value={selectedGroup.groupId}
                        disabled={editorLocked}
                        aria-invalid={selectedGroupIdInvalid}
                        aria-describedby={selectedGroupIdInvalid ? groupIdErrorId : undefined}
                        onChange={(event) => onUpdateField(selectedGroup.id, "groupId", event.currentTarget.value)}
                      />
                      {selectedGroupIdInvalid ? (
                        <small id={groupIdErrorId}>{t("分组 ID 必须填写。", "Group ID is required.")}</small>
                      ) : null}
                    </label>
                    <label className="nt-field">
                      <span>{t("分组名称", "Group name")}</span>
                      <input
                        className="nt-input"
                        value={selectedGroup.name}
                        disabled={editorLocked}
                        onChange={(event) => onUpdateField(selectedGroup.id, "name", event.currentTarget.value)}
                      />
                    </label>
                    <label className="nt-field">
                      <span>{t("计费倍率", "Billing multiplier")}</span>
                      <input
                        className="nt-input"
                        inputMode="decimal"
                        value={selectedGroup.billingMultiplier}
                        disabled={editorLocked}
                        aria-invalid={selectedGroupBillingInvalid}
                        aria-describedby={selectedGroupBillingInvalid ? billingErrorId : undefined}
                        onChange={(event) =>
                          onUpdateField(selectedGroup.id, "billingMultiplier", event.currentTarget.value)
                        }
                      />
                      {selectedGroupBillingInvalid ? (
                        <small id={billingErrorId}>
                          {t(
                            "分组计费倍率必须是大于等于 0 的数字。",
                            "Group billing multiplier must be a number greater than or equal to 0.",
                          )}
                        </small>
                      ) : null}
                    </label>
                    <label className="nt-field nt-field--wide">
                      <span>{t("描述", "Description")}</span>
                      <input
                        className="nt-input"
                        value={selectedGroup.description}
                        disabled={editorLocked}
                        onChange={(event) =>
                          onUpdateField(selectedGroup.id, "description", event.currentTarget.value)
                        }
                      />
                    </label>
                    <label className="nt-field nt-field--wide">
                      <span>{t("备注", "Notes")}</span>
                      <textarea
                        className="nt-input nt-textarea"
                        value={selectedGroup.notes}
                        disabled={editorLocked}
                        onChange={(event) => onUpdateField(selectedGroup.id, "notes", event.currentTarget.value)}
                      />
                    </label>
                  </div>
                </article>

                <article className="nt-card nt-card--panel nt-group-panel nt-group-panel--summary">
                  <div className="nt-section__head">
                    <div>
                      <p className="nt-kicker">// Membership summary</p>
                      <h3>{t("成员摘要", "Membership summary")}</h3>
                    </div>
                  </div>
                  <div className="nt-ledger-stats nt-ledger-stats--compact">
                    <article className="nt-card nt-card--stat nt-ledger-stat">
                      <span>{t("成员数量", "Members")}</span>
                      <strong>{selectedDirectoryItem?.memberCount ?? 0}</strong>
                    </article>
                    <article className="nt-card nt-card--stat nt-ledger-stat">
                      <span>{t("服务商覆盖", "Providers")}</span>
                      <strong>{selectedDirectoryItem?.providerLabels.length ?? 0}</strong>
                    </article>
                    <article className="nt-card nt-card--stat nt-ledger-stat">
                      <span>{t("模型覆盖", "Models")}</span>
                      <strong>{selectedDirectoryItem?.modelLabels.length ?? 0}</strong>
                    </article>
                  </div>
                  <div className="nt-ledger-chip-list">
                    {selectedDirectoryItem?.providerLabels.map((provider) => (
                      <span className="nt-chip" key={`${selectedDirectoryItem.rowId}:${provider}`}>
                        {provider}
                      </span>
                    ))}
                  </div>
                </article>

                <article
                  className="nt-card nt-card--panel nt-group-panel nt-group-panel--members"
                  aria-label={t("成员管理", "Member management")}
                >
                  <div className="nt-section__head">
                    <div>
                      <p className="nt-kicker">// Members</p>
                      <h3>{t("成员管理", "Member management")}</h3>
                    </div>
                  </div>
                  <div className="nt-ledger-toolbar nt-ledger-toolbar--groups">
                    <label className="nt-field nt-field--wide">
                      <span>{t("筛选候选账号", "Filter candidate accounts")}</span>
                      <input
                        className="nt-input"
                        type="search"
                        value={memberQuery}
                        placeholder={t("账号、Provider 或模型", "Account, provider, or model")}
                        onChange={(event) => onMemberQueryChange(event.currentTarget.value)}
                      />
                    </label>
                    <label className="nt-field">
                      <span>{t("候选范围", "Candidate scope")}</span>
                      <select
                        className="nt-select"
                        value={memberMode}
                        onChange={(event) =>
                          onMemberModeChange(event.currentTarget.value as "all" | "members" | "ungrouped")
                        }
                      >
                        <option value="all">{t("全部账号", "All accounts")}</option>
                        <option value="members">{t("当前成员", "Current members")}</option>
                        <option value="ungrouped">{t("仅未分组账号", "Ungrouped only")}</option>
                      </select>
                    </label>
                  </div>
                  <section className="nt-group-members" aria-label={t("成员管理", "Member management")}>
                    {memberCandidates.map((candidate) => (
                      <div className="nt-group-member-row" key={`${selectedGroup.id}:${candidate.accountId}`}>
                        <div className="nt-ledger-cell nt-ledger-cell--account">
                          <strong>{candidate.displayName}</strong>
                          <span>{candidate.accountId}</span>
                        </div>
                        <div className="nt-ledger-cell">
                          <strong>{candidate.providerLabel}</strong>
                          <span>{candidate.vendorLabel}</span>
                        </div>
                        <div className="nt-ledger-cell">
                          <span className={candidate.enabled ? "nt-badge nt-badge--success" : "nt-badge nt-badge--warning"}>
                            {candidate.enabled ? t("已启用", "Enabled") : t("已停用", "Disabled")}
                          </span>
                        </div>
                        <div className="nt-ledger-cell">
                          <div className="nt-ledger-chip-list">
                            {candidate.groupLabels.length > 0 ? (
                              candidate.groupLabels.map((group) => (
                                <span className="nt-chip" key={`${candidate.accountId}:${group}`}>
                                  {group}
                                </span>
                              ))
                            ) : (
                              <span>{t("未分组", "Ungrouped")}</span>
                            )}
                          </div>
                        </div>
                        <div className="nt-ledger-cell nt-ledger-cell--actions">
                          <ActionTooltip
                            label={
                              candidate.selected
                                ? t(`移除 ${candidate.displayName}`, `Remove ${candidate.displayName}`)
                                : t(`加入 ${candidate.displayName}`, `Add ${candidate.displayName}`)
                            }
                          >
                            <button
                              className={`nt-icon-action nt-icon-action--member${candidate.selected ? " nt-icon-action--danger" : " nt-icon-action--success"}`}
                              type="button"
                              disabled={editorLocked}
                              aria-label={candidate.selected ? t(`移除 ${candidate.displayName}`, `Remove ${candidate.displayName}`) : t(`加入 ${candidate.displayName}`, `Add ${candidate.displayName}`)}
                              onClick={() => onToggleMember(selectedGroup.id, candidate.accountId)}
                            >
                              {candidate.selected ? (
                                <UserMinus size={16} aria-hidden="true" />
                              ) : (
                                <UserPlus size={16} aria-hidden="true" />
                              )}
                              <span>{candidate.selected ? t("移除", "Remove") : t("加入", "Add")}</span>
                            </button>
                          </ActionTooltip>
                        </div>
                      </div>
                    ))}
                    {memberCandidates.length === 0 ? (
                      <p className="nt-empty">
                        {t("没有匹配的候选账号。", "No candidate accounts match this filter.")}
                      </p>
                    ) : null}
                  </section>
                </article>
              </>
            ) : null}
          </section>
        </section>
      )}
    </div>
  );
}
