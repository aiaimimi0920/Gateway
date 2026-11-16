import { Pencil, Trash2 } from "lucide-react";
import { IconButton, Switch } from "@neuro/ui";

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
    <footer className="nt-entitlement-group-card__actions nt-pool-card-actions">
      <Switch
        checked={card.enabled}
        disabled={editorLocked}
        aria-label={t(`${card.model} 启用状态`, `${card.model} enabled state`)}
        title={t("启用", "Enabled")}
        onChange={() => onToggleEnabled(card.model, !card.enabled)}
      />
      <IconButton variant="quiet" compact
        disabled={editorLocked}
        label={t("编辑", "Edit")}
        title={t("编辑模型", "Edit model")}
        onClick={() => onEditModel(card.model)}
      >
        <Pencil size={14} aria-hidden="true" />
      </IconButton>
      <IconButton variant="danger" compact
        disabled={editorLocked}
        label={t("删除", "Delete")}
        title={t("删除模型", "Delete model")}
        onClick={() => onDeleteModel(card.model)}
      >
        <Trash2 size={14} aria-hidden="true" />
      </IconButton>
    </footer>
  );
}
