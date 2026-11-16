import { useEffect, useMemo, useRef, useState } from "react";
import { Button, EmptyState, Notice, Switch } from "@neuro/ui";
import { DEFAULT_POOL_CARD_VIEW, visibleEntitlementCards } from "./poolCardView";
import { CredentialGroupEditDialog } from "./CredentialGroupEditDialog";
import { buildCredentialGroupCardSnapshot } from "./credentialGroupCardSnapshot";
import type { CredentialGroupsWorkspaceProps } from "./credentialGroupsWorkspaceTypes";
import { GroupDirectoryContent } from "./GroupDirectoryContent";
import "./PoolDirectory.css";

export type { CredentialGroupsWorkspaceProps, EntitlementAccountCardBridge } from "./credentialGroupsWorkspaceTypes";

/** Master/detail selection stays keyed by group ID across autosave row-ID refreshes. */
export function CredentialGroupsWorkspace(props: CredentialGroupsWorkspaceProps) {
  const { t, notice, error, editorLocked, groups, poolView = DEFAULT_POOL_CARD_VIEW, selectedGroupRowId, selectedGroup, selectedGroupMembers, accountCards, onSelectGroup, onAddGroup, onToggleEnabled, onRemoveGroup } = props;
  const [activeId, setActiveId] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const frame = useRef<HTMLDivElement>(null);
  const editButton = useRef<HTMLButtonElement>(null);
  const snapshots = useMemo(() => groups.map(group => buildCredentialGroupCardSnapshot(group, t, [])), [groups, t]);
  const visible = useMemo(() => visibleEntitlementCards(snapshots, poolView), [snapshots, poolView]);
  const active = (editing ? groups.find(group => group.rowId === selectedGroupRowId) : undefined) ?? visible.find(item => item.group.groupId === activeId)?.group ?? visible[0]?.group;
  useEffect(() => { if (editing && active) setActiveId(active.groupId); }, [editing, active?.groupId]);
  useEffect(() => {
    if (active && selectedGroupRowId !== active.rowId) onSelectGroup(active.rowId);
  }, [active?.rowId, selectedGroupRowId, onSelectGroup]);
  // Keep the editor bound to its group even when an outer search hides it.
  useEffect(() => { if (!active) setEditing(false); }, [active]);
  const ready = !!active && selectedGroupRowId === active.rowId;
  return <div className="nt-pool-directory" ref={frame} tabIndex={-1}>
    {notice}{error && <Notice tone="danger">{error}</Notice>}
    {!groups.length ? <EmptyState title={t("暂无分组", "No groups yet")}><Button disabled={editorLocked} onClick={onAddGroup}>{t("创建第一个分组", "Create first group")}</Button></EmptyState> : !active ? <EmptyState title={t("没有匹配的权益池，请调整或清除搜索。", "No matching entitlement pools. Change or clear the search.")} /> :
      <section className="nt-group-directory" aria-label={t("凭证分组管理", "Credential group administration")}>
        <nav className="nt-group-directory-nav" aria-label={t("权益分组", "Entitlement groups")}>
          {visible.map(({ group }) => <button type="button" key={group.groupId} aria-current={group.groupId === active.groupId ? "true" : undefined} onClick={() => { setActiveId(group.groupId); setEditing(false); onSelectGroup(group.rowId); }}>
            <strong>{group.name || group.groupId}</strong><small>{t(`${group.memberCount} 个账号`, `${group.memberCount} accounts`)}</small>
          </button>)}
        </nav>
        <div className="nt-group-directory-body">
          <header className="nt-directory-toolbar"><div><h2>{active.name || active.groupId}</h2><p className="n-muted">{t(`${active.providerScopes.length} 个供应商 · ${active.memberCount} 个账号`, `${active.providerScopes.length} providers · ${active.memberCount} accounts`)} · {t("计费倍率", "Billing multiplier")} {active.billingMultiplier || "—"}×</p></div>
            <div className="nt-directory-actions"><Switch aria-label={t(`${active.name || active.groupId} 启用状态`, `${active.name || active.groupId} enabled`)} checked={active.enabled} disabled={editorLocked} onChange={event => onToggleEnabled(active.rowId, event.target.checked)} />
              <Button ref={editButton} compact variant="quiet" disabled={editorLocked || !ready} onClick={() => setEditing(true)}>{t("编辑分组", "Edit group")}</Button>
              <Button compact variant="quiet" disabled={editorLocked} onClick={() => onRemoveGroup(active.rowId)}>{t("删除分组", "Delete group")}</Button>
            </div>
          </header>
          <GroupDirectoryContent key={active.groupId} group={active} members={selectedGroupMembers} ready={ready} bridge={accountCards} locked={editorLocked} t={t} />
        </div>
      </section>}
    {editing && active && ready && selectedGroup && <CredentialGroupEditDialog t={t} notice={notice} error={error} group={selectedGroup} editorLocked={editorLocked}
      selectedGroupIdInvalid={props.selectedGroupIdInvalid} selectedGroupBillingInvalid={props.selectedGroupBillingInvalid} memberCandidates={props.memberCandidates} memberQuery={props.memberQuery} memberMode={props.memberMode}
      onUpdateField={props.onUpdateField} onToggleEnabled={onToggleEnabled} onRemoveGroup={onRemoveGroup} onMemberQueryChange={props.onMemberQueryChange} onMemberModeChange={props.onMemberModeChange} onToggleMember={props.onToggleMember}
      resolveReturnFocus={() => editButton.current ?? frame.current} onClose={() => setEditing(false)} />}
  </div>;
}
