import { UserMinus, UserPlus } from "lucide-react";

import { ActionTooltip } from "../../components/ActionTooltip";
import type { AccountGroupDraftLike, GroupMemberCandidate } from "./accountManagementViewModel";
import type { TranslateFn } from "./credentialGroupsWorkspaceTypes";

type CredentialGroupMembersPanelProps = {
  t: TranslateFn;
  group: AccountGroupDraftLike;
  editorLocked: boolean;
  candidates: GroupMemberCandidate[];
  query: string;
  mode: "all" | "members" | "ungrouped";
  onQueryChange: (value: string) => void;
  onModeChange: (value: "all" | "members" | "ungrouped") => void;
  onToggleMember: (rowId: string, accountId: string) => void;
};

export function CredentialGroupMembersPanel({
  t,
  group,
  editorLocked,
  candidates,
  query,
  mode,
  onQueryChange,
  onModeChange,
  onToggleMember,
}: CredentialGroupMembersPanelProps) {
  return (
    <article
      className="nt-card nt-card--panel nt-group-panel nt-group-panel--members"
      aria-label={t("成员管理", "Member management")}
    >
      <div className="nt-section__head">
        <div>
          <h3>{t("成员管理", "Member management")}</h3>
        </div>
      </div>
      <div className="nt-ledger-toolbar nt-ledger-toolbar--groups">
        <label className="nt-field nt-field--wide">
          <span>{t("筛选候选账号", "Filter candidate accounts")}</span>
          <input
            className="nt-input"
            type="search"
            value={query}
            placeholder={t("账号、Provider 或模型", "Account, provider, or model")}
            onChange={(event) => onQueryChange(event.currentTarget.value)}
          />
        </label>
        <label className="nt-field">
          <span>{t("候选范围", "Candidate scope")}</span>
          <select
            className="nt-select"
            value={mode}
            onChange={(event) =>
              onModeChange(event.currentTarget.value as "all" | "members" | "ungrouped")
            }
          >
            <option value="all">{t("全部账号", "All accounts")}</option>
            <option value="members">{t("当前成员", "Current members")}</option>
            <option value="ungrouped">{t("仅未分组账号", "Ungrouped only")}</option>
          </select>
        </label>
      </div>
      <section className="nt-group-members" aria-label={t("成员管理", "Member management")}>
        {candidates.map((candidate) => (
          <div className="nt-group-member-row" key={`${group.id}:${candidate.accountId}`}>
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
                  candidate.groupLabels.map((label) => (
                    <span className="nt-chip" key={`${candidate.accountId}:${label}`}>
                      {label}
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
                  onClick={() => onToggleMember(group.id, candidate.accountId)}
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
        {candidates.length === 0 ? (
          <p className="nt-empty">{t("没有匹配的候选账号。", "No candidate accounts match this filter.")}</p>
        ) : null}
      </section>
    </article>
  );
}
