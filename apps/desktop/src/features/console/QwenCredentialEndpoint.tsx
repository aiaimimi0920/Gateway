import { Field, Input, Select } from "@neuro/ui";
import type { CredentialDialogValue } from "./CredentialDialog";
import { QwenEndpointFields } from "./QwenEndpointFields";
import { buildQwenEndpoint, emptyQwenEndpoint, parseQwenEndpoint } from "./qwenEndpoint";
import { changeQwenCredentialTarget } from "./qwenCredentialDraft";

export function QwenCredentialEndpoint({ provider, value, onChange, disabled, t }: {
  provider: Record<string, unknown>; value: CredentialDialogValue; onChange(value: CredentialDialogValue): void;
  disabled?: boolean; t(zh: string, en: string): string;
}) {
  const override = value.qwenEndpoint !== undefined || value.baseUrl !== "";
  const parent = typeof provider.base_url === "string" ? provider.base_url : "";
  return <div className="nt-stack">
    <Field label={t("Qwen 目标来源", "Qwen target source")}>
      <Select compact value={override ? "override" : "inherit"} disabled={disabled} onChange={(e) => {
        const draft = parseQwenEndpoint(parent) ?? emptyQwenEndpoint();
        onChange(changeQwenCredentialTarget(provider, value, e.target.value === "inherit"
          ? { baseUrl: "", qwenEndpoint: undefined } : { baseUrl: buildQwenEndpoint(draft) ?? "", qwenEndpoint: draft }));
      }}><option value="inherit">{t("继承服务商目标", "Inherit provider target")}</option>
        <option value="override">{t("账号独立目标", "Override for this account")}</option></Select></Field>
    {override ? <QwenEndpointFields value={value.qwenEndpoint ?? parseQwenEndpoint(value.baseUrl) ?? emptyQwenEndpoint()}
      disabled={disabled} t={t} onChange={(draft) => onChange(changeQwenCredentialTarget(provider, value,
        { baseUrl: buildQwenEndpoint(draft) ?? "", qwenEndpoint: draft }))} />
      : <Field label={t("Qwen 有效目标", "Effective Qwen target")}>
        <Input compact value={parent} readOnly /></Field>}
  </div>;
}
