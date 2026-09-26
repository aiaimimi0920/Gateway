import { Plus } from "lucide-react";
import { Fragment, useEffect, useMemo, useRef, useState } from "react";

import { CredentialGroupAccountsPanel } from "./CredentialGroupAccountsPanel";
import { CredentialGroupCard } from "./CredentialGroupCard";
import { CredentialGroupEditor } from "./CredentialGroupEditor";
import { CredentialGroupMembersPanel } from "./CredentialGroupMembersPanel";
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
  const [scopeDeselectionByGroup, setScopeDeselectionByGroup] = useState<
    Record<string, string[]>
  >({});
  const {
    activeMenuKey: activeAccountMenuKey,
    setActiveMenuKey: setActiveAccountMenuKey,
    registerMenuTrigger,
  } = useAccountCardMenu();
  const {
    activeMenuKey: activeCardMenuKey,
    setActiveMenuKey: setActiveCardMenuKey,
    registerMenuTrigger: registerCardMenuTrigger,
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
          activeMenuKey={activeCardMenuKey}
          onActiveMenuKeyChange={setActiveCardMenuKey}
          registerMenuTrigger={registerCardMenuTrigger}
          registerFlipButton={registerFlipButton}
          onToggleCard={toggleGroupCard}
          onFlip={toggleGroupFlip}
          onDeselectedProviderIdsChange={updateProviderScope}
          onSelectGroup={onSelectGroup}
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
    <div className="nt-stack nt-groups-workspace">
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

          <section className="nt-group-admin__detail" aria-label={t("分组详情", "Group detail")}>
            {selectedGroup && selectedGroupRowId === expandedGroupRowId ? (
              <>
                <CredentialGroupEditor
                  t={t}
                  group={selectedGroup}
                  editorLocked={editorLocked}
                  selectedGroupIdInvalid={selectedGroupIdInvalid}
                  selectedGroupBillingInvalid={selectedGroupBillingInvalid}
                  onUpdateField={onUpdateField}
                  onToggleEnabled={onToggleEnabled}
                  onRemoveGroup={onRemoveGroup}
                />
                <CredentialGroupMembersPanel
                  t={t}
                  group={selectedGroup}
                  editorLocked={editorLocked}
                  candidates={memberCandidates}
                  query={memberQuery}
                  mode={memberMode}
                  onQueryChange={onMemberQueryChange}
                  onModeChange={onMemberModeChange}
                  onToggleMember={onToggleMember}
                />
              </>
            ) : null}
          </section>
        </section>
      )}
    </div>
  );
}
