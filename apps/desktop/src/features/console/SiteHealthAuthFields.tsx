import { Button, Field, Input, Notice, Select } from "@neuro/ui";
import type { ProbePolicy } from "./siteHealth";
import { officialHealthAdapter } from "./officialHealthAdapters";

type Props = {
  policy: ProbePolicy;
  onChange(value: ProbePolicy): void;
  secret: { operation: "keep" | "replace" | "clear"; value: string };
  onSecret(value: Props["secret"]): void;
  configured: boolean;
  pending?: "replace" | "clear";
  hasSecretAccess: boolean;
  requestSecretAccess?(): void;
  t(zh: string, en: string): string;
};

export function SiteHealthAuthFields({
  policy: p,
  onChange,
  secret,
  onSecret,
  configured,
  pending,
  hasSecretAccess,
  requestSecretAccess,
  secretAccess,
  t,
}: Props & { secretAccess?: { busy: boolean; error: string | null } }) {
  const official = !!officialHealthAdapter(p.adapter);
  if (official && !configured && !pending) return <div>{t("认证方式：无需认证", "Authentication: none")}</div>;
  return (
    <div className="n-stack">
      {official && <Notice tone="warning">{t(
        "此官方适配器无需认证。原方案有已存或待提交密钥时，请授权后明确清空再保存；旧密钥不会发送到官方状态站。",
        "This public adapter needs no authentication. Explicitly authorize and clear any stored or pending token before saving; it will not be sent to the official status site.",
      )}</Notice>}
      <div className="nt-health-auth-fields">
        <Field label={t("监控认证", "Monitoring authentication")}>
          <Select
            value={p.auth.kind}
            disabled={official}
            onChange={(e) =>
              onChange({
                ...p,
                auth: {
                  kind: e.target.value as ProbePolicy["auth"]["kind"],
                  token: null,
                  header: e.target.value === "api_key" ? "x-api-key" : null,
                },
              })
            }
          >
            <option value="none">{t("无需认证", "None")}</option>
            <option value="bearer" disabled={p.adapter === "auto"}>
              Bearer
            </option>
            <option value="api_key" disabled={p.adapter === "auto"}>
              API key
            </option>
          </Select>
        </Field>
        {p.auth.kind === "api_key" && (
          <Field label="API key header">
            <Select
              value={p.auth.header ?? "x-api-key"}
              onChange={(e) =>
                onChange({ ...p, auth: { ...p.auth, header: e.target.value } })
              }
            >
              <option value="x-api-key">x-api-key</option>
              <option value="api-key">api-key</option>
            </Select>
          </Field>
        )}
        <Field label={t("监控密钥操作", "Monitoring secret operation")}>
          <Select
            value={secret.operation}
            disabled={!hasSecretAccess}
            onChange={(e) =>
              onSecret({
                operation: e.target.value as Props["secret"]["operation"],
                value: "",
              })
            }
          >
            <option value="keep">
              {t("保留（不显示原值）", "Keep (never reveal value)")}
            </option>
            <option
              value="replace"
              disabled={p.auth.kind === "none" || p.adapter === "auto"}
            >
              {t("替换", "Replace")}
            </option>
            <option value="clear">{t("清空", "Clear")}</option>
          </Select>
        </Field>
        {secret.operation === "replace" && hasSecretAccess && (
          <Field label={t("监控访问密钥", "Monitoring access token")}>
            <Input
              type="password"
              autoComplete="new-password"
              maxLength={8192}
              value={secret.value}
              onChange={(e) => onSecret({ ...secret, value: e.target.value })}
            />
          </Field>
        )}
      </div>
      <small className="n-muted" role="status">
        {pending === "replace"
          ? t(
            "当前草稿已有待提交替换密钥。",
            "A replacement is pending in the route draft.",
          )
          : pending === "clear"
            ? t(
              "当前草稿将清空密钥。",
              "The route draft will clear this token.",
            )
            : configured
              ? t("已配置监控密钥。", "Monitoring token configured.")
              : t("尚未配置监控密钥。", "No monitoring token configured.")}
      </small>
      {pending && hasSecretAccess && (
        <Button wrap
          onClick={() => onSecret({ operation: "keep", value: "" })}
        >
          {t("撤销待提交密钥操作", "Undo pending secret operation")}
        </Button>
      )}
      {!hasSecretAccess && requestSecretAccess && (
        <Button wrap
          disabled={secretAccess?.busy}
          onClick={requestSecretAccess}
        >
          {secretAccess?.busy
            ? t("授权中…", "Authorizing…")
            : t("授权编辑监控密钥", "Authorize monitoring secret edits")}
        </Button>
      )}
      {!hasSecretAccess && secretAccess?.error && (
        <Notice tone="danger">
          {secretAccess.error}
        </Notice>
      )}
      <small className="n-muted">
        {t(
          "只使用独立监控密钥，不借用业务凭据。变更地址、适配器或认证后，必须重新替换或清空原密钥。",
          "Uses a separate monitoring token, never business credentials. Changing destination, adapter or authentication requires explicit replacement or clearing.",
        )}
      </small>
    </div>
  );
}
