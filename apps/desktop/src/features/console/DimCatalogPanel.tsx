import { Button, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { DimModelCatalog } from "./dimModelCatalog";
import { useDimCatalog, type DimCatalogProps } from "./useDimCatalog";
export function DimCatalogPanel(props: DimCatalogProps & { onApply(catalog: DimModelCatalog): boolean }) {
  const { t } = useUiLocale();
  const flow = useDimCatalog(props);
  return <section className="nt-stack" aria-label={t("DIM 模型目录", "DIM model catalog")}>
    <h3>{t("DIM 模型目录", "DIM model catalog")}</h3>
    <p>{t("只向此账号的固定站点读取目录，不调用模型、不刷新登录令牌，也不直接修改配置。目录声明不代表模型调用已通过。",
      "Reads this account's fixed-site directory only, without generation, token refresh or configuration writes. Declarations are not verified calls.")}</p>
    <div><Button compact disabled={!flow.enabled || flow.busy} loading={flow.busy} onClick={() => void flow.read()}>{t("读取 DIM 目录", "Read DIM directory")}</Button>
      {flow.busy && <Button compact onClick={flow.cancel}>{t("取消读取", "Cancel read")}</Button>}</div>
    {(!props.binding || props.disabled) && <Notice>{t("请先保存或放弃修改，并确认账号身份与站点。", "Save or discard edits first and check the account identity and site.")}</Notice>}
    {flow.error && <Notice tone="danger">{t(flow.error, "Directory read failed or did not match this account and revision.")}</Notice>}
    {flow.catalog && <>
      <Notice>{t(`目录含 ${flow.catalog.models.length} 个声明。应用只更新当前表单的目录与拉取模型，保留手动模型；仍需点击保存。`,
        `${flow.catalog.models.length} declarations. Applying stages this account's directory and fetched models, preserves manual models, and still requires Save.`)}</Notice>
      <ul aria-label={t("DIM 目录结果", "DIM directory results")}>{flow.catalog.models.map((model) => <li key={model.id}>
        <strong>{model.name}</strong> <code>{model.id}</code> · {model.protocol}
      </li>)}</ul>
      <Button compact disabled={props.disabled || flow.busy || !props.secretGrant} onClick={() => {
        if (flow.catalog && props.onApply(flow.catalog)) flow.clear();
      }}>{t("应用 DIM 目录到表单", "Apply DIM directory to form")}</Button>
    </>}
  </section>;
}
