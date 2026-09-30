import { ChevronRight } from "lucide-react";
import type { TranslateFn } from "./accountCardTypes";
import type { CardModelTraffic, CardModelTrafficEntry } from "./cardModelTraffic";
import { groupCardModels } from "./modelDisplayCatalog";
import { aggregateSuccessWindows, buildProviderAvailabilityCells, formatAggregateRate } from "./providerCardMetrics";
import "./CardModelList.css";

/** Configured capability; actual upstream availability still requires a probe. */
export function CardModelList({ models, label, traffic, t }: {
  models: readonly string[];
  label: string;
  traffic?: CardModelTraffic;
  t: TranslateFn;
}) {
  const groups = groupCardModels(models);
  const count = groups.reduce((total, group) => total + group.models.length, 0);
  const renderRows = (groupModels: readonly string[]) => (
    <ul>
      {groupModels.map((model) => <ModelRow key={model} model={model}
        stats={traffic?.models.get(model)} emptyCount={traffic?.emptyRequestCount ?? null}
        countScope={traffic?.countScope ?? "retained"} t={t} />)}
    </ul>
  );
  return (
    <section className="nt-card-models" aria-label={label + t(" 可调用模型", " callable models")}>
      <div className="nt-card-models__heading" title={t(
        "根据当前路由配置展示，实际可用性以模型调用结果为准。",
        "Models from route configuration; availability depends on actual model calls.",
      )}>
        <span>{t("可调用模型", "Callable models")} <b>{count}</b></span>
        <span>{t("调用", "Calls")}</span>
        <span>{t("成功率", "Success")}</span>
      </div>
      {count ? (
        <div className="nt-card-models__scroll" tabIndex={0} aria-label={label + t(" 模型列表", " model list")}>
          {groups.map((group) => group.label ? (
            <details key={group.id} data-model-company={group.id}>
              <summary>
                <ChevronRight size={12} aria-hidden="true" />
                <span>{group.label}</span><span className="nt-card-models__count">{group.models.length}</span>
              </summary>
              {renderRows(group.models)}
            </details>
          ) : (
            <div key={group.id} data-model-company="unclassified">
              <div className="nt-card-models__unclassified">{t("未归类", "Unclassified")}</div>
              {renderRows(group.models)}
            </div>
          ))}
        </div>
      ) : <span className="nt-card-models__empty">{t("未声明模型", "No models declared")}</span>}
    </section>
  );
}

function ModelRow({ model, stats, emptyCount, countScope, t }: {
  model: string;
  stats?: CardModelTrafficEntry;
  emptyCount: number | null;
  countScope: CardModelTraffic["countScope"];
  t: TranslateFn;
}) {
  const windows = aggregateSuccessWindows([{ successWindows: stats ? [...stats.successWindows] : [] }]).slice(-4);
  const total = windows.reduce((value, window) => value + window.requests, 0);
  const success = windows.reduce((value, window) => value + window.success, 0);
  const rate = formatAggregateRate(total > 0 ? success / total : null);
  const count = stats ? stats.requestCount : emptyCount;
  const countLabel = count === null ? "—" : count.toLocaleString("en-US");
  const countHint = countScope === "retained"
    ? t("保留调用总数", "Retained calls")
    : t("近期采样调用数（当前审计样本）", "Recent calls in the current audit sample");
  const rateHint = t(`近期窗口成功率 ${success}/${total}；运行中与失败请求均计入分母，未调用不视为成功。`,
    `Recent window success ${success}/${total}; running and failed calls count in the denominator. No calls do not imply success.`);
  const cells = buildProviderAvailabilityCells(windows, 3);
  return (
    <li className="nt-card-models__row" data-card-model={model} title={`${model}\n${countHint}: ${countLabel}\n${rateHint}`}>
      <span className="nt-card-models__name" tabIndex={0} title={model}>{model}</span>
      <span data-model-metric="requests" aria-label={`${countHint}: ${countLabel}`}>{countLabel}</span>
      <span className="nt-card-models__success" role="img" aria-label={`${model}: ${rateHint} ${rate}`}>
        <span className="nt-card-models__cells" aria-hidden="true">
          {cells.map((cell) => <span key={`${cell.windowIndex}:${cell.position}`}
            className={`nt-provider-card__availability-cell nt-provider-card__availability-cell--${cell.state}`} />)}
        </span>
        <span data-model-metric="success-rate">{rate}</span>
      </span>
    </li>
  );
}
