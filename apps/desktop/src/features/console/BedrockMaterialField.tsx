import { Button, Field, Input, Notice, Select } from "@neuro/ui";
import type { BedrockMaterial } from "./bedrockMaterial";
import { BEDROCK_FIPS_REGIONS, BEDROCK_REGIONS, bedrockEndpoint, withoutBedrockSecrets } from "./bedrockMaterial";

/** Controlled secrets share the dialog's revoke/close/provider-change lifetime; no stored-key reveal. */
export function BedrockMaterialField({ value, keeping, disabled, onChange, onReplace }: {
  value: BedrockMaterial; keeping: boolean; disabled: boolean;
  onChange(value: BedrockMaterial): void; onReplace(): void;
}) {
  const settings = (change: Partial<BedrockMaterial>) => onChange({ ...withoutBedrockSecrets(value), ...change });
  const secret = (key: "apiKey" | "accessKeyId" | "secretAccessKey" | "sessionToken", label: string) => <Field label={label}>
    <Input compact className="nt-bedrock-material-secret" type="password" autoComplete="off" spellCheck={false} maxLength={32768}
      value={value[key]} disabled={disabled || keeping} onChange={(e) => onChange({ ...value, [key]: e.target.value })} />
  </Field>;
  let endpoint = "请选择区域";
  try { endpoint = bedrockEndpoint(value.region, value.fips); } catch { /* Invalid selection is shown, never replaced by another region. */ }
  return <section className="nt-stack" aria-label="Bedrock 认证材料">
    <Field label="Bedrock 认证模式">
      <Select compact value={value.authMode} disabled={disabled}
        onChange={(e) => settings({ authMode: e.target.value as BedrockMaterial["authMode"] })}>
        {!value.authMode && <option value="">保留存量模式（替换时必须明确选择）</option>}
        <option value="bedrock_bearer">Bedrock API Key</option><option value="aws_sigv4">IAM SigV4</option>
        <option value="aws_sigv4_temporary">临时 IAM / STS</option>
      </Select>
    </Field>
    <Field label="AWS 区域"><Select compact value={value.region} disabled={disabled}
      onChange={(e) => settings({ region: e.target.value, fips: value.fips && BEDROCK_FIPS_REGIONS.includes(e.target.value) })}>
      {!BEDROCK_REGIONS.includes(value.region as typeof BEDROCK_REGIONS[number]) && <option value="">请选择区域</option>}
      {BEDROCK_REGIONS.map((region) => <option key={region} value={region}>{region}</option>)}
    </Select></Field>
    <Field label="FIPS runtime"><Select compact value={value.fips ? "fips" : "standard"}
      disabled={disabled} onChange={(e) => settings({ fips: e.target.value === "fips" })}>
      <option value="standard">标准公共 runtime</option><option value="fips" disabled={!BEDROCK_FIPS_REGIONS.includes(value.region)}>FIPS runtime</option>
    </Select></Field>
    <p className="nt-bedrock-material-note">目标：{endpoint}。模式或区域变更会清空暂存材料；不自动回退区域。</p>
    {keeping ? <div className="nt-stack"><Notice>保留已保存材料，不读取或自动转换。</Notice>
      <Button compact disabled={disabled} onClick={onReplace}>重新填写完整认证材料</Button></div>
      : value.authMode === "bedrock_bearer" ? secret("apiKey", "Bedrock API Key")
        : value.authMode ? <>{secret("accessKeyId", "AWS Access Key ID")}{secret("secretAccessKey", "AWS Secret Access Key")}
          {secret("sessionToken", "AWS Session Token")}
          <p className="nt-bedrock-material-note">临时 IAM 必填 Session Token；长期 IAM 不强制。到期后需重新录入，本入口不自动取得 STS 凭据。</p></> : null}
    <p className="nt-bedrock-material-note">材料模式不代表独立额度。仅使用手动模型 ID / inference profile；本入口不查询 OpenAI 模型目录。</p>
  </section>;
}
