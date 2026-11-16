import { Button, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { NousModelCatalog } from "./nousModelCatalog";
import { useNousCatalog, type NousCatalogProps } from "./useNousCatalog";
export function NousCatalogPanel(props: NousCatalogProps & { onApply(catalog: NousModelCatalog): boolean }) {
  const { t } = useUiLocale();
  const flow = useNousCatalog(props);
  return <section className="nt-stack" aria-label={t("Nous 模型目录", "Nous model catalog")}>
    <Button compact disabled={!flow.enabled || flow.busy} loading={flow.busy} onClick={() => void flow.read()}>{t("读取 Nous 目录", "Read Nous directory")}</Button>
    {flow.busy && <Button compact onClick={flow.cancel}>{t("取消读取", "Cancel read")}</Button>}
    {flow.error && <Notice tone="danger">{t("目录读取失败或账号版本已变化。", "Directory read failed or account revision changed.")}</Notice>}
    {flow.catalog && <>
      <Notice>{t(`目录：${flow.catalog.models.length} 个模型`, `Directory: ${flow.catalog.models.length} models`)}</Notice>
      <ul aria-label={t("Nous 目录结果", "Nous directory results")} style={{ maxHeight: "12rem", overflow: "auto", overflowWrap: "anywhere" }}>
        {flow.catalog.models.map((model) => <li key={model.id}><code>{model.id}</code></li>)}
      </ul>
      <Button compact disabled={props.disabled || flow.busy || !props.secretGrant} onClick={() => {
        if (flow.catalog && props.onApply(flow.catalog)) flow.clear();
      }}>{t("应用到表单", "Apply to form")}</Button>
    </>}
  </section>;
}
