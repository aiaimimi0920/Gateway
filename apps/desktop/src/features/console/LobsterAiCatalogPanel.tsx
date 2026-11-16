import { Button, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { LobsterAiModelCatalog } from "./lobsteraiModelCatalog";
import { useLobsterAiCatalog, type LobsterAiCatalogProps } from "./useLobsterAiCatalog";
export function LobsterAiCatalogPanel(props: LobsterAiCatalogProps & { onApply(catalog: LobsterAiModelCatalog): boolean }) {
  const { t } = useUiLocale(); const flow = useLobsterAiCatalog(props);
  return <section className="n-stack" aria-label={t("LobsterAI 模型目录", "LobsterAI model catalog")}>
    <div><Button compact disabled={!flow.enabled || flow.busy} loading={flow.busy} onClick={() => void flow.read()}>{t("读取模型目录", "Read model catalog")}</Button>
      {flow.busy && <Button compact onClick={flow.cancel}>{t("取消读取", "Cancel read")}</Button>}</div>
    {props.disabled && <Notice>{t("请先保存当前修改", "Save current changes first")}</Notice>}
    {flow.error && <Notice tone="danger">{flow.error}</Notice>}
    {flow.catalog && <>
      <Notice>{t(`${flow.catalog.models.length} 个模型`, `${flow.catalog.models.length} models`)}</Notice>
      <ul style={{ maxHeight: 240, overflowY: "auto", overflowWrap: "anywhere" }} aria-label={t("目录结果", "Catalog results")}>{flow.catalog.models.map((model) => <li key={model.modelId}>
        <strong>{model.modelName ?? model.modelId}</strong> <code>{model.modelId}</code>{model.apiFormat ? ` · ${model.apiFormat}` : ""}
      </li>)}</ul>
      <Button compact disabled={props.disabled || flow.busy || !props.secretGrant} onClick={() => { if (flow.catalog && props.onApply(flow.catalog)) flow.clear(); }}>{t("应用到表单", "Apply to form")}</Button>
    </>}
  </section>;
}
