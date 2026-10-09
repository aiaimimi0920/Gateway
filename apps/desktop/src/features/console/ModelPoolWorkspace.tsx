import { Fragment, useEffect, useRef, useState, type ReactNode } from "react";

import type { EntitlementAccountCardBridge } from "./credentialGroupsWorkspaceTypes";
import type { ModelPoolCard } from "./modelPoolViewModel";
import { ModelPoolCardView } from "./ModelPoolCardView";
import { ModelPoolServingAccountsPanel } from "./ModelPoolServingAccountsPanel";
import { useAccountCardMenu } from "./ProviderAccountCard";

type TranslateFn = (zh: string, en: string) => string;

export type ModelPoolWorkspaceProps = {
  t: TranslateFn;
  notice?: ReactNode;
  editorLocked: boolean;
  models: ModelPoolCard[];
  /** Supplied by the console shell so a model's accounts reuse the pool cards. */
  accountCards?: EntitlementAccountCardBridge;
  onMoveProvider: (model: string, providerId: string, direction: "up" | "down") => void;
  onResetChain: (model: string) => void;
  /** Flips the model's own route between dispatching and standing down. */
  onToggleEnabled: (model: string, enabled: boolean) => void;
  onEditModel: (model: string) => void;
  onDeleteModel: (model: string) => void;
};

/** DOM ids have to survive model names like `gemini-2.5-pro:free`. */
function domSafeKey(value: string): string {
  return value.replace(/[^a-zA-Z0-9_-]/g, "-");
}

/**
 * Owns cross-card state and composes the model-card and serving-account owners.
 * The card owners receive state and callbacks; model routing state remains here.
 */
export function ModelPoolWorkspace({
  t,
  notice,
  editorLocked,
  models,
  accountCards,
  onMoveProvider,
  onResetChain,
  onToggleEnabled,
  onEditModel,
  onDeleteModel,
}: ModelPoolWorkspaceProps) {
  const [expandedModel, setExpandedModel] = useState<string | null>(null);
  /**
   * Chain selection per card, stored as the complement so an untouched card
   * means "every provider". It lives here because the expanded account panel
   * below the card filters by the same choice.
   */
  const [chainDeselectionByModel, setChainDeselectionByModel] = useState<Record<string, string[]>>(
    {},
  );
  const {
    activeMenuKey: activeAccountMenuKey,
    setActiveMenuKey: setActiveAccountMenuKey,
    registerMenuTrigger,
  } = useAccountCardMenu();
  const [flippedModels, setFlippedModels] = useState<string[]>([]);
  const [flipAnnouncement, setFlipAnnouncement] = useState("");
  const flipButtonRefs = useRef(new Map<string, HTMLButtonElement>());
  const pendingFlipFocusRef = useRef<{ model: string; side: "front" | "back" } | null>(null);

  const toggleModelCard = (model: string) => {
    setExpandedModel((current) => (current === model ? null : model));
  };

  const toggleModelFlip = (model: string, side: "front" | "back") => {
    pendingFlipFocusRef.current = { model, side };
    setFlippedModels((current) =>
      side === "back"
        ? current.includes(model)
          ? current
          : [...current, model]
        : current.filter((candidate) => candidate !== model),
    );
    setFlipAnnouncement(
      side === "back"
        ? t(`${model} 卡牌已翻到背面`, `${model} card flipped to the back`)
        : t(`${model} 卡牌已翻回正面`, `${model} card flipped to the front`),
    );
  };

  useEffect(() => {
    const pendingFocus = pendingFlipFocusRef.current;
    if (!pendingFocus) {
      return;
    }
    flipButtonRefs.current.get(`${pendingFocus.model}:${pendingFocus.side}`)?.focus();
    pendingFlipFocusRef.current = null;
  }, [flippedModels]);

  return (
    <div className="nt-stack nt-model-pool-workspace">
      {notice}
      <p className="nt-visually-hidden" role="status" aria-live="polite" aria-atomic="true">
        {flipAnnouncement}
      </p>

      {models.length === 0 ? (
        <article className="nt-card nt-card--panel nt-empty-state">
          <h2>{t("暂无模型", "No models yet")}</h2>
          <p className="nt-copy">
            {t(
              "还没有账号声明可用模型。先在凭据池里挂载账号并填写支持的模型，模型池会据此生成卡牌。",
              "No account declares a usable model yet. Attach accounts in the credential pool and list their supported models; the model pool builds its cards from that.",
            )}
          </p>
        </article>
      ) : (
        <section className="nt-group-admin" aria-label={t("模型池管理", "Model pool administration")}>
          <div
            className="nt-entitlement-group-grid"
            role="region"
            aria-label={t("模型卡牌", "Model cards")}
          >
            {models.map((card) => {
              const expanded = expandedModel === card.model;
              const flipped = flippedModels.includes(card.model);
              const deselectedProviderIds = chainDeselectionByModel[card.model] ?? [];
              const chainNarrowed = deselectedProviderIds.length > 0;
              const selectedLinks = card.chain.filter(
                (link) => !deselectedProviderIds.includes(link.providerId),
              );
              // The card's provider selection and its attached account panel are one scope.
              const scopedAccountIds = (chainNarrowed ? selectedLinks : card.chain).flatMap(
                (link) => link.accountIds,
              );
              const accountPanelId = `model-pool-accounts-${domSafeKey(card.model)}`;
              return (
                <Fragment key={card.rowId}>
                  <ModelPoolCardView
                    t={t}
                    card={card}
                    expanded={expanded}
                    flipped={flipped}
                    editorLocked={editorLocked}
                    accountPanelId={accountPanelId}
                    deselectedProviderIds={deselectedProviderIds}
                    registerFlipButton={(side, node) => {
                      const refKey = `${card.model}:${side}`;
                      if (node) {
                        flipButtonRefs.current.set(refKey, node);
                      } else {
                        flipButtonRefs.current.delete(refKey);
                      }
                    }}
                    onToggleCard={() => toggleModelCard(card.model)}
                    onFlip={(side) => toggleModelFlip(card.model, side)}
                    onDeselectedProviderIdsChange={(next) =>
                      setChainDeselectionByModel((current) => ({
                        ...current,
                        [card.model]: next,
                      }))
                    }
                    onMoveProvider={(providerId, direction) =>
                      onMoveProvider(card.model, providerId, direction)
                    }
                    onToggleEnabled={onToggleEnabled}
                    onEditModel={onEditModel}
                    onDeleteModel={onDeleteModel}
                    onResetChain={onResetChain}
                  />

                  {expanded ? (
                    <ModelPoolServingAccountsPanel
                      t={t}
                      model={card.model}
                      accountPanelId={accountPanelId}
                      selectedLinks={selectedLinks}
                      scopedAccountIds={scopedAccountIds}
                      chainNarrowed={chainNarrowed}
                      accountCards={accountCards}
                      editorLocked={editorLocked}
                      activeMenuKey={activeAccountMenuKey}
                      onActiveMenuKeyChange={setActiveAccountMenuKey}
                      registerMenuTrigger={registerMenuTrigger}
                    />
                  ) : null}
                </Fragment>
              );
            })}
          </div>
        </section>
      )}
    </div>
  );
}
