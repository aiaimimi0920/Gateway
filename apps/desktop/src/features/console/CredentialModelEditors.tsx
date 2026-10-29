import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { CredentialDialogValue } from "./CredentialDialog";
import { editModelSources } from "./credentialModelSelection";

export function CredentialModelEditors({ value, disabled, canFetch, busy, onFetch, onChange }: {
  value: CredentialDialogValue; disabled: boolean; canFetch: boolean; busy: boolean;
  onFetch(): void; onChange(value: CredentialDialogValue): void;
}) {
  const { t } = useUiLocale();
  return <div className="nt-credential-row nt-credential-models">
    <div className="nt-credential-model-actions">
      <span>{t("支持模型", "Supported models")}</span>
      <button className="nt-btn nt-btn--outline" type="button" disabled={disabled || !canFetch}
        onClick={onFetch}>{busy ? t("拉取中…", "Fetching…") : t("拉取模型", "Fetch models")}</button>
      <span className="nt-credential-model-status" role="status" aria-atomic="true">
        {value.modelCatalog ? <>{t("目录已发现", "Catalog discovered")} {value.modelCatalog.models.length}</> : null}
      </span>
    </div>
    <div className="nt-credential-model-editors">
      <label><span>{t("拉取模型", "Fetched models")}</span><textarea className="nt-textarea" rows={6}
        aria-label={t("拉取模型列表", "Fetched model list")} maxLength={526_336} disabled={disabled} value={value.fetchedModelsText ?? ""}
        onChange={(event) => onChange(editModelSources(value, event.target.value, value.manualModelsText ?? ""))} /></label>
      <label><span>{t("手动模型", "Manual models")}</span><textarea className="nt-textarea" rows={6}
        aria-label={t("手动模型列表", "Manual model list")} maxLength={526_336} disabled={disabled} value={value.manualModelsText ?? ""}
        placeholder={t("每行一个模型，也支持逗号分隔", "One model per line; commas are also accepted")}
        onChange={(event) => onChange(editModelSources(value, value.fetchedModelsText ?? "", event.target.value))} /></label>
    </div>
  </div>;
}
