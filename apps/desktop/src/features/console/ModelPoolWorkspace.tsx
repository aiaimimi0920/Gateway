import { Fragment, useMemo, useState, type ReactNode } from "react";
import { Button, EmptyState, Switch } from "@neuro/ui";
import { ChevronDown, ChevronRight } from "lucide-react";
import { DEFAULT_POOL_CARD_VIEW, visibleModelCards } from "./poolCardView";
import type { ProviderPoolView } from "./providerPoolView";
import type { EntitlementAccountCardBridge } from "./credentialGroupsWorkspaceTypes";
import type { ModelPoolCard } from "./modelPoolViewModel";
import { ModelPriorityList } from "./ModelPriorityList";
import { ModelPoolServingAccountsPanel } from "./ModelPoolServingAccountsPanel";
import { ScopePager } from "./EntitlementScopePager";
import "./PoolDirectory.css";

type TranslateFn = (zh: string, en: string) => string;
const MODELS_PER_PAGE = 24;
export type ModelPoolWorkspaceProps = {
  t: TranslateFn;
  notice?: ReactNode;
  editorLocked: boolean;
  models: ModelPoolCard[];
  poolView?: ProviderPoolView;
  accountCards?: EntitlementAccountCardBridge;
  onMoveProvider: (model: string, providerId: string, direction: "up" | "down" | number) => void;
  onResetChain: (model: string) => void;
  onToggleEnabled: (model: string, enabled: boolean) => void;
  onEditModel: (model: string) => void;
  onDeleteModel: (model: string) => void;
};

/** Compact directory over the existing route editor; no parallel persistence owner. */
export function ModelPoolWorkspace({ t, notice, editorLocked, models, poolView = DEFAULT_POOL_CARD_VIEW, accountCards, onMoveProvider, onResetChain, onToggleEnabled, onEditModel, onDeleteModel }: ModelPoolWorkspaceProps) {
  const [expandedModel, setExpandedModel] = useState<string | null>(null);
  const [accountScope, setAccountScope] = useState<{ model: string; provider: string | null } | null>(null);
  const visibleModels = useMemo(() => visibleModelCards(models, poolView), [models, poolView]);
  const [pagination, setPagination] = useState({ ...poolView, page: 1 });
  const totalPages = Math.max(1, Math.ceil(visibleModels.length / MODELS_PER_PAGE));
  const page = pagination.query === poolView.query && pagination.sort === poolView.sort ? Math.min(pagination.page, totalPages) : 1;
  if (pagination.query !== poolView.query || pagination.sort !== poolView.sort || pagination.page !== page) setPagination({ ...poolView, page });
  const pageModels = visibleModels.slice((page - 1) * MODELS_PER_PAGE, page * MODELS_PER_PAGE);
  return <div className="nt-pool-directory">
    {notice}
    <section aria-label={t("模型池管理", "Model pool administration")}>
      <div className="nt-directory-toolbar"><span className="n-muted">{t(`${visibleModels.length} 个模型`, `${visibleModels.length} models`)}</span>
        <ScopePager t={t} page={page} totalPages={totalPages} navLabel={t("模型列表分页", "Model list pagination")} onPage={next => setPagination({ ...poolView, page: next })} />
      </div>
      <div className="nt-directory-surface"><table className="nt-directory-table" aria-label={t("模型目录", "Model directory")}>
        <thead><tr><th style={{ width: "40%" }}>{t("模型 / 调用 ID", "Model / API ID")}</th><th>{t("来源", "Sources")}</th><th>{t("关联账号", "Accounts")}</th><th>{t("状态", "Status")}</th></tr></thead>
        <tbody>{pageModels.map(card => {
          const expanded = expandedModel === card.model;
          const selectedLinks = card.chain.filter(link => accountScope?.provider == null || accountScope.provider === link.providerId);
          const panelId = `model-sources-${encodeURIComponent(card.model)}`;
          return <Fragment key={card.model}>
            <tr className="nt-directory-model-row" data-expanded={expanded}>
              <td><button type="button" className="nt-directory-model-name" aria-expanded={expanded} aria-controls={expanded ? panelId : undefined} onClick={() => { setExpandedModel(expanded ? null : card.model); setAccountScope(null); }}>
                {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}<span title={card.model}>{card.model}</span></button></td>
              <td><span>{t(`${card.chain.length} 个来源`, `${card.chain.length} sources`)}</span><small className="nt-directory-secondary" title={card.chain.map(link => link.providerLabel).join(" · ")}>{card.chain.map(link => link.providerLabel).join(" · ")}</small></td>
              <td>{card.accountCount}</td><td><Switch aria-label={t(`${card.model} 启用状态`, `${card.model} enabled`)} checked={card.enabled} disabled={editorLocked} onChange={event => onToggleEnabled(card.model, event.target.checked)} /></td>
            </tr>
            {expanded && <tr className="nt-directory-expanded"><td colSpan={4}>
              <section id={panelId} aria-label={t(`${card.model} 供应商优先级`, `${card.model} provider priority`)}>
                <div className="nt-directory-toolbar"><strong>{t("供应商优先级", "Provider priority")}</strong><div className="nt-directory-actions">
                  <Button compact variant="quiet" disabled={editorLocked || !card.pinned} onClick={() => onResetChain(card.model)}>{t("恢复继承顺序", "Restore inherited order")}</Button>
                  <Button compact variant="quiet" disabled={editorLocked} onClick={() => onEditModel(card.model)}>{t("编辑模型", "Edit model")}</Button>
                  <Button compact variant="quiet" disabled={editorLocked} onClick={() => onDeleteModel(card.model)}>{t("删除模型", "Delete model")}</Button>
                </div></div>
                <ModelPriorityList chain={card.chain} locked={editorLocked} t={t} onMove={(id, direction) => onMoveProvider(card.model, id, direction)} onAccounts={provider => setAccountScope({ model: card.model, provider })} />
                <div className="nt-directory-toolbar"><span className="n-muted">{t("拖动调整顺序，越靠上越优先", "Drag to reorder; higher entries are tried first")}</span><Button compact variant="quiet" onClick={() => setAccountScope(accountScope?.model === card.model ? null : { model: card.model, provider: null })}>{accountScope?.model === card.model ? t("收起账号", "Hide accounts") : t("查看全部账号", "View all accounts")}</Button></div>
                {accountScope?.model === card.model && <ModelPoolServingAccountsPanel t={t} model={card.model} accountPanelId={`${panelId}-accounts`} selectedLinks={selectedLinks} scopedAccountIds={[...new Set(selectedLinks.flatMap(link => link.accountIds))]} chainNarrowed={accountScope.provider !== null} accountCards={accountCards} editorLocked={editorLocked} />}
              </section>
            </td></tr>}
          </Fragment>;
        })}</tbody>
      </table>{!pageModels.length && <EmptyState title={models.length ? t("没有匹配的模型，请调整或清除搜索。", "No matching models. Change or clear the search.") : t("暂无模型", "No models yet")} />}</div>
    </section>
  </div>;
}
