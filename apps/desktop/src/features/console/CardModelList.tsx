import { ChevronRight } from "lucide-react";
import type { TranslateFn } from "./accountCardTypes";
import type { CardModelTraffic, CardModelTrafficEntry } from "./cardModelTraffic";
import { aggregateCardModelTraffic } from "./cardModelTraffic";
import { AvailabilityHeatmap } from "./AvailabilityHeatmap";
import { recentAvailabilityWindows } from "./availabilityWindows";
import { cardModelDisplayName, groupCardModels } from "./modelDisplayCatalog";
import { aggregateSuccessWindows, buildProviderAvailabilityCells, formatAggregateRate } from "./providerCardMetrics";
import "./CardModelList.css";

/** Catalog visibility and successful retained calls are separate evidence. */
export function CardModelList({ models, label, traffic, t }: {
  models: readonly string[];
  label: string;
  traffic?: CardModelTraffic;
  t: TranslateFn;
}) {
  const groups = groupCardModels(models);
  const discovered = new Set(traffic?.catalogModels ?? []);
  const count = groups.reduce((total, group) => total + group.models.length, 0);
  const renderRows = (groupModels: readonly string[]) => (
    <ul>
      {groupModels.map((model) => <ModelRow key={model} model={model}
        discovered={discovered.has(model)}
        stats={traffic?.models.get(model)} emptyCount={traffic?.emptyRequestCount ?? null}
        t={t} />)}
    </ul>
  );
  return (
    <section className="nt-card-models" aria-label={label + t(" 模型池", " model pool")}>
      <div className="nt-card-models__heading" title={t(
        "根据当前路由配置展示，实际可用性以模型调用结果为准。",
        "Models from route configuration; availability depends on actual model calls.",
      )}>
        <span>{t("模型池", "Model pool")} <b>{count}</b></span>
        <span>{t("调用数", "Calls")}</span>
        <span>{t("热度图", "Heatmap")}</span>
        <span>{t("成功率", "Success")}</span>
      </div>
      {count ? (
        <div className="nt-card-models__scroll" tabIndex={0} aria-label={label + t(" 模型列表", " model list")}>
          {groups.map((group) => (
            <details key={group.id} data-model-company={group.id}>
              <summary className="nt-card-models__row">
                <span className="nt-card-models__company-name" title={`${group.label}(${group.models.length})`}>
                  <ChevronRight size={12} aria-hidden="true" />
                  <span className="nt-card-models__company-label">
                    <span className="nt-card-models__company-text">{group.label}</span>
                    <span className="nt-card-models__count">({group.models.length})</span>
                  </span>
                </span>
                <ModelMetrics label={group.label} stats={aggregateCardModelTraffic(group.models, traffic)} t={t} />
              </summary>
              {renderRows(group.models)}
            </details>
          ))}
        </div>
      ) : <span className="nt-card-models__empty">{t("未声明模型", "No models declared")}</span>}
    </section>
  );
}

function ModelRow({ model, stats, emptyCount, discovered, t }: {
  model: string;
  stats?: CardModelTrafficEntry;
  emptyCount: number | null;
  discovered: boolean;
  t: TranslateFn;
}) {
  const evidence = (stats?.successCount ?? 0) > 0 ? null
    : (stats?.requestCount ?? 0) > 0 ? t("尚无成功", "No success recorded")
    : discovered ? t("目录已发现", "Catalog discovered") : t("未验证", "Unverified");
  return (
    <li className="nt-card-models__row" data-card-model={model} title={model}>
      <span className="nt-card-models__name" tabIndex={0} title={evidence ? `${model} · ${evidence}` : model} aria-label={evidence ? `${model} · ${evidence}` : model}>
        <span className="nt-card-models__model-label" title={model} aria-label={model}>{cardModelDisplayName(model)}</span>
        {evidence && <span className="nt-card-models__evidence" title={t("保留历史不保证当前权限或可用性。", "Retained history does not guarantee current access or availability.")}>{evidence}</span>}
      </span>
      <ModelMetrics label={model} stats={stats} emptyCount={emptyCount} t={t} />
    </li>
  );
}

function ModelMetrics({ label, stats, emptyCount = null, t }: {
  label: string;
  stats?: CardModelTrafficEntry;
  emptyCount?: number | null;
  t: TranslateFn;
}) {
  const windows = recentAvailabilityWindows(aggregateSuccessWindows([{ successWindows: stats ? [...stats.successWindows] : [] }]));
  const count = stats ? stats.requestCount : emptyCount;
  const success = stats?.successCount ?? null;
  const rate = formatAggregateRate(count !== null && count > 0 && success !== null ? success / count : null);
  const countLabel = count === null ? "—" : count.toLocaleString("en-US");
  const countHint = t("调用总次数（保留历史）", "Total calls (retained history)");
  const rateHint = t(`总成功率 ${success ?? "—"}/${countLabel}；按保留历史统计，运行中、取消和失败请求均计入分母。`,
    `Total success ${success ?? "—"}/${countLabel}; all retained calls including running, cancelled and failed calls count in the denominator.`);
  const cells = buildProviderAvailabilityCells(windows);
  return (
    <>
      <span data-model-metric="requests" title={`${countHint}: ${countLabel}`} aria-label={`${countHint}: ${countLabel}`}>{countLabel}</span>
      <span data-model-metric="quality">
        <AvailabilityHeatmap cells={cells} compact label={`${label} ${t("热度图", "heatmap")}`} t={t} />
      </span>
      <span data-model-metric="success-rate" title={rateHint} aria-label={`${label}: ${rateHint}`}>{rate}</span>
    </>
  );
}
