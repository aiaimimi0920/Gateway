import { Plus } from "lucide-react";
import { Fragment, useEffect, useMemo, useRef, useState } from "react";

import { CredentialGroupAccountsPanel } from "./CredentialGroupAccountsPanel";
import { CredentialGroupCard } from "./CredentialGroupCard";
import { CredentialGroupEditDialog } from "./CredentialGroupEditDialog";
import {
  buildCredentialGroupCardSnapshot,
  type CredentialGroupCardSnapshot,
} from "./credentialGroupCardSnapshot";
import type { CredentialGroupsWorkspaceProps } from "./credentialGroupsWorkspaceTypes";
import { useAccountCardMenu } from "./ProviderAccountCard";

export type {
  CredentialGroupsWorkspaceProps,
  EntitlementAccountCardBridge,
} from "./credentialGroupsWorkspaceTypes";

type FlipSide = "front" | "back";

export function CredentialGroupsWorkspace({
  t,
  notice,
  error,
  editorLocked,
  groups,
  selectedGroupRowId,
  selectedGroup,
  selectedGroupIdInvalid,
  selectedGroupBillingInvalid,
  selectedGroupMembers,
  memberCandidates,
  memberQuery,
  memberMode,
  accountCards,
  onSelectGroup,
  onAddGroup,
  onUpdateField,
  onToggleEnabled,
  onRemoveGroup,
  onMemberQueryChange,
  onMemberModeChange,
  onToggleMember,
}: CredentialGroupsWorkspaceProps) {
  const [expandedGroupRowId, setExpandedGroupRowId] = useState<string | null>(null);
  const [editedGroupRowId, setEditedGroupRowId] = useState<string | null>(null);
  const editReturnFocusRef = useRef<HTMLElement | null>(null);
  const editGroupIdRef = useRef<string | null>(null);
  const workspaceRef = useRef<HTMLDivElement>(null);
  const editingGroup = editedGroupRowId
    ? groups.find((group) => group.rowId === editedGroupRowId) ??
      groups.find((group) => group.groupId === editGroupIdRef.current)
    : null;
  const [scopeDeselectionByGroup, setScopeDeselectionByGroup] = useState<
    Record<string, string[]>
  >({});
  const {
    activeMenuKey: activeAccountMenuKey,
    setActiveMenuKey: setActiveAccountMenuKey,
    registerMenuTrigger,
  } = useAccountCardMenu();
  const [flippedGroupRowIds, setFlippedGroupRowIds] = useState<string[]>([]);
  const [groupFlipAnnouncement, setGroupFlipAnnouncement] = useState("");
  const groupFlipButtonRefs = useRef(new Map<string, HTMLButtonElement>());
  const pendingGroupFlipFocusRef = useRef<{ rowId: string; side: FlipSide } | null>(null);
  const flippedGroupRowIdSet = useMemo(
    () => new Set(flippedGroupRowIds),
    [flippedGroupRowIds],
  );
  const groupCardSnapshots = useMemo(
    () =>
      groups.map((group) =>
        buildCredentialGroupCardSnapshot(
          group,
          t,
          scopeDeselectionByGroup[group.rowId] ?? [],
        ),
      ),
    [groups, scopeDeselectionByGroup, t],
  );

  const toggleGroupCard = (rowId: string) => {
    if (expandedGroupRowId === rowId) {
      setExpandedGroupRowId(null);
      return;
    }
    onSelectGroup(rowId);
    setExpandedGroupRowId(rowId);
  };

  const editGroup = (rowId: string) => {
    if (editorLocked) return;
    editReturnFocusRef.current = document.activeElement as HTMLElement;
    editGroupIdRef.current = groups.find((group) => group.rowId === rowId)?.groupId ?? null;
    onSelectGroup(rowId);
    setEditedGroupRowId(rowId);
  };

  useEffect(() => {
    if (!editedGroupRowId) return;
    if (!editingGroup) { setEditedGroupRowId(null); return; }
    // Autosave reloads the document with fresh draft row IDs; retain the open group.
    editGroupIdRef.current = editingGroup.groupId;
    if (editingGroup.rowId !== editedGroupRowId) setEditedGroupRowId(editingGroup.rowId);
    if (selectedGroupRowId !== editingGroup.rowId) onSelectGroup(editingGroup.rowId);
  }, [editedGroupRowId, editingGroup, onSelectGroup, selectedGroupRowId]);

  const restoreEditFocus = () => {
    const card = Array.from(workspaceRef.current?.querySelectorAll<HTMLElement>("[data-entitlement-group-card]") ?? [])
      .find((node) => node.dataset.entitlementGroupCard === editGroupIdRef.current);
    const edit = Array.from(card?.querySelectorAll<HTMLButtonElement>("button") ?? [])
      .find((button) => button.title === t("编辑权益组", "Edit entitlement group"));
    const target = editReturnFocusRef.current?.isConnected ? editReturnFocusRef.current : edit;
    if (target && !target.matches(":disabled")) target.focus();
    else workspaceRef.current?.focus();
  };

  const toggleGroupFlip = (rowId: string, label: string, side: FlipSide) => {
    pendingGroupFlipFocusRef.current = { rowId, side };
    setFlippedGroupRowIds((current) =>
      side === "back"
        ? current.includes(rowId)
          ? current
          : [...current, rowId]
        : current.filter((candidate) => candidate !== rowId),
    );
    setGroupFlipAnnouncement(
      side === "back"
        ? t(`${label} 卡牌已翻到背面`, `${label} card flipped to the back`)
        : t(`${label} 卡牌已翻回正面`, `${label} card flipped to the front`),
    );
  };

  const registerFlipButton = (
    rowId: string,
    side: FlipSide,
    node: HTMLButtonElement | null,
  ) => {
    const refKey = `${rowId}:${side}`;
    if (node) {
      groupFlipButtonRefs.current.set(refKey, node);
    } else {
      groupFlipButtonRefs.current.delete(refKey);
    }
  };

  const updateProviderScope = (rowId: string, providerIds: string[]) => {
    setScopeDeselectionByGroup((current) => ({ ...current, [rowId]: providerIds }));
  };

  useEffect(() => {
    const pendingFocus = pendingGroupFlipFocusRef.current;
    if (!pendingFocus) {
      return;
    }
    groupFlipButtonRefs.current.get(`${pendingFocus.rowId}:${pendingFocus.side}`)?.focus();
    pendingGroupFlipFocusRef.current = null;
  }, [flippedGroupRowIds]);

  const renderGroup = (snapshot: CredentialGroupCardSnapshot) => {
    const { group } = snapshot;
    const expanded = expandedGroupRowId === group.rowId;
    const flipped = flippedGroupRowIdSet.has(group.rowId);
    const accountsReady = expanded && group.rowId === selectedGroupRowId;

    return (
      <Fragment key={group.rowId}>
        <CredentialGroupCard
          t={t}
          snapshot={snapshot}
          expanded={expanded}
          flipped={flipped}
          editorLocked={editorLocked}
          registerFlipButton={registerFlipButton}
          onToggleCard={toggleGroupCard}
          onFlip={toggleGroupFlip}
          onDeselectedProviderIdsChange={updateProviderScope}
          onEditGroup={editGroup}
          onToggleEnabled={onToggleEnabled}
          onRemoveGroup={onRemoveGroup}
        />
        {expanded ? (
          <CredentialGroupAccountsPanel
            t={t}
            snapshot={snapshot}
            accountsReady={accountsReady}
            editorLocked={editorLocked}
            members={selectedGroupMembers}
            accountCards={accountCards}
            activeMenuKey={activeAccountMenuKey}
            onActiveMenuKeyChange={setActiveAccountMenuKey}
            registerMenuTrigger={registerMenuTrigger}
          />
        ) : null}
      </Fragment>
    );
  };

  return (
    <div className="nt-stack nt-groups-workspace" ref={workspaceRef} tabIndex={-1}>
      {notice}
      <p className="nt-visually-hidden" role="status" aria-live="polite" aria-atomic="true">
        {groupFlipAnnouncement}
      </p>

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
            <button
              className="nt-btn nt-btn--secondary"
              type="button"
              disabled={editorLocked}
              onClick={onAddGroup}
            >
              <Plus size={15} aria-hidden="true" />
              {t("创建第一个分组", "Create first group")}
            </button>
          </div>
        </article>
      ) : (
        <section
          className="nt-group-admin"
          aria-label={t("凭证分组管理", "Credential group administration")}
        >
          <div
            className="nt-entitlement-group-grid"
            role="region"
            aria-label={t("权益组卡牌", "Entitlement group cards")}
          >
            {groupCardSnapshots.map(renderGroup)}
          </div>

        </section>
      )}
      {editingGroup ? (
        <CredentialGroupEditDialog
          t={t}
          notice={notice}
          error={error}
          group={selectedGroup?.id === editingGroup.rowId ? selectedGroup : {
            ...editingGroup, id: editingGroup.rowId, providerCredentialIds: [],
          }}
          editorLocked={editorLocked}
          selectedGroupIdInvalid={selectedGroupIdInvalid}
          selectedGroupBillingInvalid={selectedGroupBillingInvalid}
          memberCandidates={selectedGroupRowId === editingGroup.rowId ? memberCandidates : []}
          memberQuery={memberQuery}
          memberMode={memberMode}
          onUpdateField={onUpdateField}
          onToggleEnabled={onToggleEnabled}
          onRemoveGroup={onRemoveGroup}
          onMemberQueryChange={onMemberQueryChange}
          onMemberModeChange={onMemberModeChange}
          onToggleMember={onToggleMember}
          onRestoreFocus={restoreEditFocus}
          onClose={() => setEditedGroupRowId(null)}
        />
      ) : null}
    </div>
  );
}
