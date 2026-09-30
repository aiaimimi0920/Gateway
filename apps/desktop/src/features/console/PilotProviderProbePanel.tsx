import { Play } from "lucide-react";
import type { ConsoleProviderProbeResponse } from "../../api/contracts";
import type { AccountsLedgerPilotSection } from "./accountsLedgerTypes";

type PilotProviderProbePanelProps = {
  t: (zh: string, en: string) => string;
  section: AccountsLedgerPilotSection;
  closePilotActionDialog: () => void;
  providerProbeBusy: boolean;
  providerProbeError: string | null;
  providerProbeResponse: ConsoleProviderProbeResponse | null;
  draftDirty: boolean;
  draftMatchesActiveRevision: boolean;
  startProviderProbe: () => Promise<void>;
};

export function PilotProviderProbePanel({
  t,
  section,
  closePilotActionDialog,
  providerProbeBusy,
  providerProbeError,
  providerProbeResponse,
  draftDirty,
  draftMatchesActiveRevision,
  startProviderProbe,
}: PilotProviderProbePanelProps) {
  const modelTest = section.providerId === "nvidia" || section.providerPreset === "nvidia-openai";
  return (
    <div className="nt-stack">
      <article className="nt-pilot-dialog__hero">
        <div className="nt-pilot-dialog__hero-icon">
          <Play size={20} aria-hidden="true" />
        </div>
        <div className="nt-pilot-dialog__hero-copy">
          <strong>{section.providerLabel}</strong>
          <span>
            {modelTest ? t(
              "逐个使用 NVIDIA 账号调用其配置的模型，收到有效回复才通过。每个账号最多 256 个输出 token，最长等待 60 秒。",
              "Call a configured model with each NVIDIA credential. Only a valid reply passes; up to 256 output tokens and 60 seconds per account.",
            ) : t(
              "顺序测试该服务商的全部已生效账号；每个账号只访问适配器定义的健康端点，不发送模型生成请求。",
              "Sequentially test every active account using only the adapter-defined health endpoint, without sending model-generation requests.",
            )}
          </span>
        </div>
      </article>

      {providerProbeBusy ? (
        <div className="nt-pilot-stats-state" role="status">
          {t("正在逐个测试服务商账号...", "Testing provider accounts one by one...")}
        </div>
      ) : null}
      {providerProbeError ? (
        <div className="nt-banner nt-banner--danger" role="alert">
          {providerProbeError}
        </div>
      ) : null}
      {providerProbeResponse ? (
        <>
          <div className="nt-pilot-stats-overview-grid">
            <article className="nt-pilot-stat-card">
              <span>{t("账号总数", "Accounts")}</span>
              <strong>{providerProbeResponse.result.totalCount}</strong>
            </article>
            <article className="nt-pilot-stat-card nt-pilot-stat-card--emerald">
              <span>{t("通过", "Passed")}</span>
              <strong>{providerProbeResponse.result.passedCount}</strong>
            </article>
            <article className="nt-pilot-stat-card nt-pilot-stat-card--danger">
              <span>{t("失败", "Failed")}</span>
              <strong>{providerProbeResponse.result.failedCount}</strong>
            </article>
            <article className="nt-pilot-stat-card nt-pilot-stat-card--blue">
              <span>{t("不支持", "Unsupported")}</span>
              <strong>{providerProbeResponse.result.unsupportedCount}</strong>
            </article>
          </div>
          <div className="nt-stack">
            {providerProbeResponse.result.results.map((result) => (
              <article className="nt-pilot-metric-card" key={result.credentialId}>
                <h3>{result.credentialId}</h3>
                <dl className="nt-pilot-metric-list">
                  <div>
                    <dt>{t("结果", "Result")}</dt>
                    <dd>{result.status}</dd>
                  </div>
                  <div>
                    <dt>{t("单点", "Probe point")}</dt>
                    <dd>{result.probePoint}</dd>
                  </div>
                  <div>
                    <dt>{t("说明", "Message")}</dt>
                    <dd>{result.message}</dd>
                  </div>
                </dl>
              </article>
            ))}
          </div>
        </>
      ) : null}

      <div className="dialog-actions">
        <button
          className="nt-btn nt-btn--secondary"
          type="button"
          onClick={closePilotActionDialog}
        >
          {t("关闭", "Close")}
        </button>
        <button
          className="nt-btn nt-btn--primary"
          type="button"
          disabled={providerProbeBusy || draftDirty || !draftMatchesActiveRevision}
          onClick={() => void startProviderProbe()}
        >
          {providerProbeBusy
            ? t("测试中...", "Testing...")
            : t("测试全部账号", "Test all accounts")}
        </button>
      </div>
    </div>
  );
}
