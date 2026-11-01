import type { ProviderCatalogTemplate } from "./providerCatalog";
import { templateAdaptationComplete, templateAdaptationPaused } from "./providerTemplateAdaptation";

export type CatalogCategoryFilter = "standard" | "custom";
type Props = {
  t(zh: string, en: string): string;
  category: CatalogCategoryFilter;
  filteredTemplates: readonly ProviderCatalogTemplate[];
  selectedTemplateId: string;
  setCategory(value: CatalogCategoryFilter): void;
  selectTemplate(template: ProviderCatalogTemplate): void;
};
export function ProviderCatalogDirectory({ t, category, filteredTemplates, selectedTemplateId, setCategory, selectTemplate }: Props) {
  return <aside className="nt-provider-catalog__directory" aria-label={t("服务商目录", "Provider directory")}>
    <div className="nt-provider-catalog__categories" role="group" aria-label={t("模板分类", "Template group")}>
      {(["standard", "custom"] as const).map((id) => <button type="button" key={id} aria-pressed={category === id}
        onClick={() => setCategory(id)}>{id === "standard" ? t("标准模板", "Standard templates") : t("自定义模板", "Custom templates")}</button>)}
    </div>
    <div className="nt-provider-catalog__list">
      {filteredTemplates.map((template) => {
        const complete = templateAdaptationComplete(template.id);
        const paused = !complete && templateAdaptationPaused(template.id);
        const status = complete ? t("新调用流程已验收", "New call flow verified")
          : paused ? t("未完成（已暂停）", "Incomplete (paused)") : undefined;
        return <button type="button" key={template.id} aria-pressed={template.id === selectedTemplateId}
          data-adaptation={complete ? "complete" : "pending"}
          data-development={paused ? "paused" : undefined}
          title={status}
          className={`nt-provider-catalog__item${template.id === selectedTemplateId ? " nt-provider-catalog__item--selected" : ""}${complete ? " nt-provider-catalog__item--adapted" : ""}`}
          onClick={() => selectTemplate(template)}>{template.providerId}
          {status && <span className="nt-visually-hidden">{status}</span>}
        </button>;
      })}
    </div>
  </aside>;
}
