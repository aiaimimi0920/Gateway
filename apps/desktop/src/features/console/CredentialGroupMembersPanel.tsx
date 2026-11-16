import { UserMinus, UserPlus } from "lucide-react";

import { ActionTooltip, Badge, Button, DataTable, EmptyState, Field, Input, Select, type TableColumn } from "@neuro/ui";
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
  const columns: TableColumn<GroupMemberCandidate>[] = [
    {
      key: "account", header: t("账号", "Account"), width: "26%", cell: candidate => <div className="nt-ledger-cell nt-ledger-cell--account">
        <strong>{candidate.displayName}</strong><span>{candidate.accountId}</span>
      </div>
    },
    {
      key: "provider", header: "Provider", width: "22%", cell: candidate => <div className="nt-ledger-cell">
        <strong>{candidate.providerLabel}</strong><span>{candidate.vendorLabel}</span>
      </div>
    },
    {
      key: "state", header: t("状态", "Status"), width: "16%", cell: candidate => <Badge tone={candidate.enabled ? "success" : "warning"}>
        {candidate.enabled ? t("已启用", "Enabled") : t("已停用", "Disabled")}
      </Badge>
    },
    {
      key: "groups", header: t("分组", "Groups"), width: "20%", cell: candidate => <div className="n-row">
        {candidate.groupLabels.length ? candidate.groupLabels.map(label => <Badge key={`${candidate.accountId}:${label}`}>{label}</Badge>) : t("未分组", "Ungrouped")}
      </div>
    },
    {
      key: "actions", header: t("操作", "Actions"), width: "16%", cell: candidate => <ActionTooltip
        label={candidate.selected ? t(`移除 ${candidate.displayName}`, `Remove ${candidate.displayName}`) : t(`加入 ${candidate.displayName}`, `Add ${candidate.displayName}`)}>
        <Button compact wrap variant={candidate.selected ? "danger" : "positive"} disabled={editorLocked}
          aria-label={candidate.selected ? t(`移除 ${candidate.displayName}`, `Remove ${candidate.displayName}`) : t(`加入 ${candidate.displayName}`, `Add ${candidate.displayName}`)}
          onClick={() => onToggleMember(group.id, candidate.accountId)}>
          {candidate.selected ? <UserMinus size={16} aria-hidden="true" /> : <UserPlus size={16} aria-hidden="true" />}
          <span>{candidate.selected ? t("移除", "Remove") : t("加入", "Add")}</span>
        </Button>
      </ActionTooltip>
    },
  ];
  return (
    <section
      className="nt-group-members-editor"
      aria-label={t("成员管理", "Member management")}
    >
      <div className="nt-section__head">
        <div>
          <h3>{t("成员管理", "Member management")}</h3>
        </div>
      </div>
      <div className="nt-group-members-editor__filters">
        <Field label={t("筛选候选账号", "Filter candidate accounts")}>
          <Input
            type="search"
            value={query}
            placeholder={t("账号、Provider 或模型", "Account, provider, or model")}
            onChange={(event) => onQueryChange(event.currentTarget.value)}
          />
        </Field>
        <Field label={t("候选范围", "Candidate scope")}>
          <Select
            value={mode}
            onChange={(event) =>
              onModeChange(event.currentTarget.value as "all" | "members" | "ungrouped")
            }
          >
            <option value="all">{t("全部账号", "All accounts")}</option>
            <option value="members">{t("当前成员", "Current members")}</option>
            <option value="ungrouped">{t("仅未分组账号", "Ungrouped only")}</option>
          </Select>
        </Field>
      </div>
      <DataTable label={t("候选账号", "Candidate accounts")} rows={candidates} columns={columns}
        getRowKey={candidate => candidate.accountId}
        empty={<EmptyState title={t("没有匹配的候选账号。", "No candidate accounts match this filter.")} />} />
    </section>
  );
}
