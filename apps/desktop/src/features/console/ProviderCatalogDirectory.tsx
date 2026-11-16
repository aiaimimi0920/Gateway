import { Button, Input, Panel } from "@neuro/ui";
import { useState } from "react";
import { PROVIDER_ACCESS_KINDS, providerAccessKindLabel, type ProviderAccessKind } from "./providerAccessKind";
import type { ProviderCatalogTemplate } from "./providerCatalog";
import { templateAdaptationComplete, templateAdaptationPaused } from "./providerTemplateAdaptation";

export type CatalogCategoryFilter = "all" | ProviderAccessKind;
type Props = {
  t(zh: string, en: string): string;
  category: CatalogCategoryFilter;
  filteredTemplates: readonly ProviderCatalogTemplate[];
  selectedTemplateId: string;
  setCategory(value: CatalogCategoryFilter): void;
  selectTemplate(template: ProviderCatalogTemplate): void;
};
export function ProviderCatalogDirectory({ t, category, filteredTemplates, selectedTemplateId, setCategory, selectTemplate }: Props) {
  const [query, setQuery] = useState("");
  return <Panel className="nt-provider-catalog__directory" aria-label={t("服务商目录", "Provider directory")}>
    <div className="nt-provider-catalog__categories" role="group" aria-label={t("模板分类", "Template group")}>
      {(["all", ...PROVIDER_ACCESS_KINDS] as const).map(value => <Button compact variant="quiet" key={value} aria-pressed={category === value} onClick={() => setCategory(value)}>{value === "all" ? t("全部", "All") : providerAccessKindLabel(value, t)}</Button>)}
    </div>
    <Input compact type="search" aria-label={t("搜索服务商模板", "Search provider templates")} placeholder={t("搜索服务商", "Search providers")} value={query} onChange={e => setQuery(e.target.value)} />
    <div className="nt-provider-catalog__list">
      {filteredTemplates.filter(template => `${template.providerId} ${template.labelZh} ${template.labelEn}`.toLowerCase().includes(query.trim().toLowerCase())).map((template) => {
        const complete = templateAdaptationComplete(template.id);
        const paused = !complete && templateAdaptationPaused(template.id);
        const status = complete ? t("新调用流程已验收", "New call flow verified")
          : paused ? t("未完成（已暂停）", "Incomplete (paused)") : undefined;
        return <Button variant="quiet" compact key={template.id} aria-pressed={template.id === selectedTemplateId}
          data-adaptation={complete ? "complete" : "pending"}
          data-development={paused ? "paused" : undefined}
          title={status}
          className="nt-provider-catalog__item"
          onClick={() => selectTemplate(template)}>{template.providerId}
          {status && <span className="nt-visually-hidden">{status}</span>}
        </Button>;
      })}
    </div>
  </Panel>;
}
