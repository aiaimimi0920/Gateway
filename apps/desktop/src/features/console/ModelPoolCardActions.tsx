import { Database, Ellipsis, ListOrdered, Pencil, RotateCcw, Trash2 } from "lucide-react";

import type { ModelPoolCard } from "./modelPoolViewModel";
import type { useAccountCardMenu } from "./ProviderAccountCard";

type ModelPoolCardActionsProps = {
  t: (zh: string, en: string) => string;
  card: ModelPoolCard;
  expanded: boolean;
  editorLocked: boolean;
  accountPanelId: string;
  menu: ReturnType<typeof useAccountCardMenu>;
  onToggleEnabled: (model: string, enabled: boolean) => void;
  onEditModel: (model: string) => void;
  onDeleteModel: (model: string) => void;
  onResetChain: (model: string) => void;
  toggleModelFlip: (model: string, side: "front" | "back") => void;
  toggleModelCard: (model: string) => void;
};

/** Renders the four card actions without taking ownership of either menu lifetime. */
export function ModelPoolCardActions({
  t, card, expanded, editorLocked, accountPanelId, menu,
  onToggleEnabled, onEditModel, onDeleteModel, onResetChain,
  toggleModelFlip, toggleModelCard,
}: ModelPoolCardActionsProps) {
  const cardMenuKey = `model-pool-card:${card.model}`;
  const {
    activeMenuKey: activeCardMenuKey,
    setActiveMenuKey: setActiveCardMenuKey,
    registerMenuTrigger: registerCardMenuTrigger,
  } = menu;
  return (
    <footer className="nt-entitlement-group-card__actions">
      <button
        className={
          card.enabled
            ? "nt-entitlement-group-card__action nt-entitlement-group-card__action--icon nt-entitlement-group-card__action--active"
            : "nt-entitlement-group-card__action nt-entitlement-group-card__action--icon"
        }
        type="button"
        role="switch"
        aria-checked={card.enabled}
        disabled={editorLocked}
        aria-label={t(`${card.model} 启用状态`, `${card.model} enabled state`)}
        title={t("启用", "Enabled")}
        onClick={() => onToggleEnabled(card.model, !card.enabled)}
      >
        <span className="nt-switch__track" aria-hidden="true">
          <span className="nt-switch__thumb" />
        </span>
      </button>
      <button
        className="nt-entitlement-group-card__action nt-entitlement-group-card__action--icon"
        type="button"
        disabled={editorLocked}
        aria-label={t("编辑", "Edit")}
        title={t("编辑模型", "Edit model")}
        onClick={() => onEditModel(card.model)}
      >
        <Pencil size={14} aria-hidden="true" />
      </button>
      <button
        className="nt-entitlement-group-card__action nt-entitlement-group-card__action--icon nt-entitlement-group-card__action--danger"
        type="button"
        disabled={editorLocked}
        aria-label={t("删除", "Delete")}
        title={t("删除模型", "Delete model")}
        onClick={() => onDeleteModel(card.model)}
      >
        <Trash2 size={14} aria-hidden="true" />
      </button>
      <div className="nt-pilot-menu" data-pilot-menu-key={cardMenuKey}>
        <button
          className="nt-entitlement-group-card__action nt-entitlement-group-card__action--icon"
          type="button"
          ref={(node) => registerCardMenuTrigger(cardMenuKey, node)}
          aria-haspopup="menu"
          aria-expanded={activeCardMenuKey === cardMenuKey}
          aria-label={t(`${card.model} 更多操作`, `${card.model} more actions`)}
          title={t("更多", "More")}
          onClick={() =>
            setActiveCardMenuKey((current) =>
              current === cardMenuKey ? null : cardMenuKey,
            )
          }
        >
          <Ellipsis size={14} aria-hidden="true" />
        </button>
        {activeCardMenuKey === cardMenuKey ? (
          <div className="nt-pilot-menu__panel" role="menu">
            <button
              className="nt-pilot-menu__item"
              role="menuitem"
              type="button"
              onClick={() => {
                setActiveCardMenuKey(null);
                toggleModelFlip(card.model, "back");
              }}
            >
              <ListOrdered size={15} aria-hidden="true" />
              <span>{t("调整优先级", "Reorder chain")}</span>
            </button>
            <button
              className="nt-pilot-menu__item"
              role="menuitem"
              type="button"
              disabled={editorLocked || !card.pinned}
              title={
                card.pinned
                  ? undefined
                  : t("当前已是默认顺序", "Already the inherited order")
              }
              onClick={() => {
                setActiveCardMenuKey(null);
                onResetChain(card.model);
              }}
            >
              <RotateCcw size={15} aria-hidden="true" />
              <span>{t("重置优先级", "Reset chain")}</span>
            </button>
            <button
              className="nt-pilot-menu__item"
              role="menuitem"
              type="button"
              aria-expanded={expanded}
              aria-controls={accountPanelId}
              onClick={() => {
                setActiveCardMenuKey(null);
                toggleModelCard(card.model);
              }}
            >
              <Database size={15} aria-hidden="true" />
              <span>
                {expanded
                  ? t("收起可用账号", "Hide serving accounts")
                  : t("显示可用账号", "Show serving accounts")}
              </span>
            </button>
          </div>
        ) : null}
      </div>
    </footer>
  );
}
