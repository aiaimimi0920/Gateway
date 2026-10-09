import { Search } from "lucide-react";
import type { ProviderCatalogCategory, ProviderCatalogTemplate } from "./providerCatalog";

export type CatalogCategoryFilter = "all" | ProviderCatalogCategory;

type ProviderCatalogDirectoryProps = {
  t(zh: string, en: string): string;
  query: string;
  category: CatalogCategoryFilter;
  filteredTemplates: readonly ProviderCatalogTemplate[];
  selectedTemplateId: string;
  setQuery(value: string): void;
  setCategory(value: CatalogCategoryFilter): void;
  selectTemplate(template: ProviderCatalogTemplate): void;
};

export function ProviderCatalogDirectory({
  t, query, category, filteredTemplates, selectedTemplateId,
  setQuery, setCategory, selectTemplate,
}: ProviderCatalogDirectoryProps) {
  return (
    <aside className="nt-provider-catalog__directory" aria-label={t("服务商目录", "Provider directory")}>
      <label className="nt-field nt-provider-catalog__search">
        <span>{t("搜索服务商", "Search providers")}</span>
        <span className="nt-provider-catalog__search-control">
          <Search size={15} aria-hidden="true" />
          <input
            className="nt-input"
            value={query}
            placeholder={t("名称、Provider ID 或 preset", "Name, provider ID, or preset")}
            onChange={(event) => setQuery(event.currentTarget.value)}
          />
        </span>
      </label>
      <label className="nt-field">
        <span>{t("目录分类", "Catalog category")}</span>
        <select
          className="nt-select"
          value={category}
          onChange={(event) => setCategory(event.currentTarget.value as CatalogCategoryFilter)}
        >
          <option value="all">{t("全部内建模板", "All built-in templates")}</option>
          <option value="mainstream">{t("主流官方服务", "Mainstream official services")}</option>
          <option value="aggregator">{t("模型聚合服务", "Model aggregators")}</option>
          <option value="search">{t("搜索与抓取", "Search and fetch")}</option>
          <option value="third-party-compatible">
            {t("第三方兼容服务", "Third-party compatible services")}
          </option>
        </select>
      </label>
      <div className="nt-provider-catalog__list">
        {filteredTemplates.map((template) => {
          const selected = template.id === selectedTemplateId;
          return (
            <button
              className={
                selected
                  ? "nt-provider-catalog__item nt-provider-catalog__item--selected"
                  : "nt-provider-catalog__item"
              }
              type="button"
              aria-pressed={selected}
              key={template.id}
              onClick={() => selectTemplate(template)}
            >
              <span className="nt-provider-catalog__item-copy">
                <strong>{t(template.labelZh, template.labelEn)}</strong>
                <small>{template.providerId}</small>
              </span>
              {template.category === "third-party-compatible" ? (
                <span className="nt-badge nt-badge--info">
                  {t("第三方兼容", "Third-party")}
                </span>
              ) : null}
            </button>
          );
        })}
        {filteredTemplates.length === 0 ? (
          <p className="nt-empty">{t("没有匹配的服务商模板。", "No provider templates match.")}</p>
        ) : null}
      </div>
    </aside>
  );
}
