import { Pencil, Trash2 } from "lucide-react";

import type { ModelPoolCard } from "./modelPoolViewModel";

type ModelPoolCardActionsProps = {
  t: (zh: string, en: string) => string;
  card: ModelPoolCard;
  editorLocked: boolean;
  onToggleEnabled: (model: string, enabled: boolean) => void;
  onEditModel: (model: string) => void;
  onDeleteModel: (model: string) => void;
};

/** Renders the model card's direct actions. */
export function ModelPoolCardActions({
  t, card, editorLocked,
  onToggleEnabled, onEditModel, onDeleteModel,
}: ModelPoolCardActionsProps) {
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
    </footer>
  );
}
