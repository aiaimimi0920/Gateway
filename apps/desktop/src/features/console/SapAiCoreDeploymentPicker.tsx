import { Button, Input, Notice, Select } from "@neuro/ui";
import { useEffect, useRef, useState } from "react";
import type { DiscoverSapDeployments, SapDeploymentCatalog, SapDeploymentProtocol } from "../../api/sapAiCoreDeployments";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { CatalogField } from "./ProviderCatalogFields";
import { sapEndpoint } from "./sapAiCore";

type Props = {
  resourceGroup: string;
  protocol: SapDeploymentProtocol;
  baseUrl: string;
  disabled: boolean;
  hasSecretAccess: boolean;
  onRequestSecretAccess(): void;
  discover: DiscoverSapDeployments;
  select(url: string): void;
};

export function SapAiCoreDeploymentPicker({ resourceGroup, protocol, baseUrl, disabled, hasSecretAccess, onRequestSecretAccess, discover, select }: Props) {
  const { t } = useUiLocale();
  const [key, setKey] = useState("");
  const [result, setResult] = useState<SapDeploymentCatalog | null>(null);
  const [state, setState] = useState<"idle" | "loading" | "error">("idle");
  const pending = useRef<AbortController | null>(null);
  useEffect(() => {
    setKey(""); setResult(null); setState("idle");
    return () => { pending.current?.abort(); pending.current = null; };
  }, [discover, hasSecretAccess]);
  const read = async () => {
    if (disabled || !hasSecretAccess || pending.current || !key.trim() || !/^[A-Za-z0-9_-]{1,253}$/.test(resourceGroup)) return;
    const request = new AbortController();
    pending.current = request;
    // Query-only material stays outside the configuration draft and is never saved with the pool.
    setKey(""); setResult(null); setState("loading");
    try {
      const catalog = await discover({ serviceKey: key.trim(), resourceGroup,
        ...(protocol !== "azure" ? { protocol } : {}) }, request.signal);
      if (request.signal.aborted) return;
      setResult({ ...catalog, deployments: catalog.deployments.filter((entry) => sapEndpoint(entry.deploymentUrl)?.deployment === entry.id) });
      setState("idle");
    } catch {
      if (!request.signal.aborted) setState("error");
    } finally {
      if (pending.current === request) pending.current = null;
    }
  };
  if (!hasSecretAccess) return <div><Button type="button" disabled={disabled} onClick={onRequestSecretAccess}>
    {t("授权查询部署", "Authorize deployment lookup")}
  </Button></div>;
  const chosen = result?.deployments.find((entry) => entry.deploymentUrl === baseUrl)?.id ?? "";
  const selectLabel = protocol === "bedrock"
    ? t("选择已确认支持 Converse 的部署", "Select a deployment confirmed to support Converse")
    : protocol === "orchestration" ? t("选择 Orchestration 部署", "Select an Orchestration deployment") : t("选择部署", "Select deployment");
  return <>
    <CatalogField name={t("查询用 Service Key JSON", "Service Key JSON for lookup")} wide>
      <Input type="password" autoComplete="off" spellCheck={false} maxLength={65536} value={key}
        onKeyDown={(event) => { if (event.key === "Enter") event.preventDefault(); }}
        disabled={disabled || state === "loading"} onChange={(event) => { setKey(event.target.value); setResult(null); setState("idle"); }} />
    </CatalogField>
    <div><Button type="button" disabled={disabled || state === "loading" || !key.trim() || !/^[A-Za-z0-9_-]{1,253}$/.test(resourceGroup)}
      onClick={() => void read()}>{state === "loading" ? t("查询中…", "Looking up…") : t("查询部署", "Look up deployments")}</Button></div>
    {state === "error" && <Notice tone="danger">{t("查询失败。请检查 Service Key、资源组和权限后重试。", "Lookup failed. Check the service key, resource group and permissions.")}</Notice>}
    {result && result.deployments.length === 0 && <Notice tone="info">{protocol === "bedrock" ? t("未找到运行中的 Bedrock 部署。", "No running Bedrock deployment found.")
      : protocol === "orchestration" ? t("未找到运行中的 Orchestration 部署。", "No running Orchestration deployment found.")
      : t("未找到受支持的运行中 Chat 部署。", "No supported running Chat deployments found.")}</Notice>}
    {result?.truncated && <Notice tone="info">{t("已达到查询上限；未列出的部署可手动填写。", "Lookup limit reached. Unlisted deployments can be entered manually.")}</Notice>}
    {!!result?.deployments.length && <CatalogField name={selectLabel} wide>
      <Select value={chosen} disabled={disabled} onChange={(event) => {
        const entry = result.deployments.find(({ id }) => id === event.target.value);
        if (entry) select(entry.deploymentUrl);
      }}>
        <option value="" disabled>{selectLabel}</option>
        {result.deployments.map(({ id, configurationName, modelName, modelVersion }) => {
          const label = modelName ? `${modelName}${modelVersion ? ` (${modelVersion})` : ""}` : configurationName;
          return <option key={id} value={id}>{label ? `${label} · ${id}` : id}</option>;
        })}
      </Select>
    </CatalogField>}
  </>;
}
