import { Database, GalleryHorizontalEnd, Layers, RotateCcw } from "lucide-react";

import type { ModelPoolCard as ModelPoolCardData } from "./modelPoolViewModel";
import { ModelPoolCardActions } from "./ModelPoolCardActions";
import { ModelPoolCardMetrics } from "./ModelPoolCardMetrics";
import { ModelPoolChainBoard } from "./ModelPoolChainBoard";

type TranslateFn = (zh: string, en: string) => string;
type FlipSide = "front" | "back";

export type ModelPoolCardViewProps = {
  t: TranslateFn;
  card: ModelPoolCardData;
  expanded: boolean;
  flipped: boolean;
  editorLocked: boolean;
  accountPanelId: string;
  deselectedProviderIds: readonly string[];
  registerFlipButton: (side: FlipSide, node: HTMLButtonElement | null) => void;
  onToggleCard: () => void;
  onFlip: (side: FlipSide) => void;
  onDeselectedProviderIdsChange: (next: string[]) => void;
  onMoveProvider: (providerId: string, direction: "up" | "down") => void;
  onToggleEnabled: (model: string, enabled: boolean) => void;
  onEditModel: (model: string) => void;
  onDeleteModel: (model: string) => void;
  onResetChain: (model: string) => void;
};

/** Owns one model card's two faces while its workspace owns shared state. */
export function ModelPoolCardView({
  t,
  card,
  expanded,
  flipped,
  editorLocked,
  accountPanelId,
  deselectedProviderIds,
  registerFlipButton,
  onToggleCard,
  onFlip,
  onDeselectedProviderIdsChange,
  onMoveProvider,
  onToggleEnabled,
  onEditModel,
  onDeleteModel,
  onResetChain,
}: ModelPoolCardViewProps) {
  return (
    <article
      className={`nt-entitlement-group-card nt-model-pool-card${flipped ? " nt-entitlement-group-card--flipped" : ""}${expanded ? " nt-entitlement-group-card--expanded" : ""}`}
      data-model-pool-card={card.model}
      data-model-pool-card-side={flipped ? "back" : "front"}
      aria-label={t(`${card.model} 模型卡牌`, `${card.model} model card`)}
    >
      <div className="nt-entitlement-group-card__inner">
        <div
          className="nt-entitlement-group-card__face nt-entitlement-group-card__front"
          aria-hidden={flipped}
          inert={flipped ? true : undefined}
        >
          <header className="nt-entitlement-group-card__head">
            <span className="nt-entitlement-group-card__icon" aria-hidden="true">
              <Layers size={17} />
            </span>
            <button
              className="nt-entitlement-group-card__toggle"
              type="button"
              aria-label={card.model}
              aria-expanded={expanded}
              aria-controls={accountPanelId}
              onClick={onToggleCard}
            >
              <strong>{card.model}</strong>
            </button>
            <div className="nt-entitlement-group-card__head-actions">
              <button
                className="nt-icon-action nt-entitlement-group-card__library-toggle"
                type="button"
                title={
                  expanded
                    ? t("收起可用账号", "Hide serving accounts")
                    : t("显示可用账号", "Show serving accounts")
                }
                aria-label={
                  expanded
                    ? t(
                        `收起 ${card.model} 可用账号`,
                        `Hide ${card.model} serving accounts`,
                      )
                    : t(
                        `显示 ${card.model} 可用账号`,
                        `Show ${card.model} serving accounts`,
                      )
                }
                aria-expanded={expanded}
                aria-controls={accountPanelId}
                onClick={onToggleCard}
              >
                <Database size={15} aria-hidden="true" />
              </button>
              <button
                className="nt-icon-action nt-entitlement-group-card__flip"
                type="button"
                title={t(
                  `翻面调整 ${card.model} 优先级`,
                  `Flip ${card.model} to reorder its chain`,
                )}
                aria-label={t(
                  `翻面调整 ${card.model} 优先级`,
                  `Flip ${card.model} to reorder its chain`,
                )}
                aria-pressed={false}
                ref={(node) => registerFlipButton("front", node)}
                onClick={() => onFlip("back")}
              >
                <GalleryHorizontalEnd size={16} aria-hidden="true" />
              </button>
            </div>
          </header>

          <ModelPoolCardMetrics card={card} t={t} />

          <ModelPoolCardActions
            t={t}
            card={card}
            editorLocked={editorLocked}
            onToggleEnabled={onToggleEnabled}
            onEditModel={onEditModel}
            onDeleteModel={onDeleteModel}
          />
        </div>

        <div
          className="nt-entitlement-group-card__face nt-entitlement-group-card__back"
          aria-hidden={!flipped}
          inert={!flipped ? true : undefined}
        >
          <header className="nt-entitlement-group-card__back-head">
            <div className="nt-entitlement-group-card__back-title">
              <strong>{card.model}</strong>
            </div>
            <div className="nt-entitlement-group-card__head-actions">
              <button
                className="nt-icon-action"
                type="button"
                aria-label={t("重置优先级", "Reset chain")}
                title={card.pinned ? t("重置优先级", "Reset chain") : t("当前已是默认顺序", "Already the inherited order")}
                disabled={editorLocked || !card.pinned}
                onClick={() => onResetChain(card.model)}
              >
                <RotateCcw size={15} aria-hidden="true" />
              </button>
              <button
                className="nt-icon-action nt-entitlement-group-card__library-toggle"
                type="button"
                title={
                  expanded
                    ? t("收起可用账号", "Hide serving accounts")
                    : t("显示可用账号", "Show serving accounts")
                }
                aria-label={
                  expanded
                    ? t(
                        `收起 ${card.model} 可用账号`,
                        `Hide ${card.model} serving accounts`,
                      )
                    : t(
                        `显示 ${card.model} 可用账号`,
                        `Show ${card.model} serving accounts`,
                      )
                }
                aria-expanded={expanded}
                aria-controls={accountPanelId}
                onClick={onToggleCard}
              >
                <Database size={15} aria-hidden="true" />
              </button>
              <button
                className="nt-icon-action nt-entitlement-group-card__flip"
                type="button"
                title={t(
                  `翻回 ${card.model} 卡牌正面`,
                  `Flip ${card.model} back to the front`,
                )}
                aria-label={t(
                  `翻回 ${card.model} 卡牌正面`,
                  `Flip ${card.model} back to the front`,
                )}
                aria-pressed={true}
                ref={(node) => registerFlipButton("back", node)}
                onClick={() => onFlip("front")}
              >
                <GalleryHorizontalEnd size={16} aria-hidden="true" />
              </button>
            </div>
          </header>

          <section
            className="nt-entitlement-group-card__back-body"
            aria-label={t(
              `${card.model} 服务商优先级`,
              `${card.model} provider priority`,
            )}
          >
            <ModelPoolChainBoard
              t={t}
              cardKey={card.rowId}
              model={card.model}
              chain={card.chain}
              editorLocked={editorLocked}
              deselectedProviderIds={deselectedProviderIds}
              onDeselectedProviderIdsChange={onDeselectedProviderIdsChange}
              onMoveProvider={onMoveProvider}
            />
          </section>
        </div>
      </div>
    </article>
  );
}
