import { Play } from "lucide-react";
import type { ConsoleCredentialProbeResult } from "../../api/contracts";
import type { AccountsLedgerPilotAccount } from "./accountsLedgerTypes";
import type { RouteManagedAccount } from "./routeAccountCatalog";

type PilotAccountProbePanelProps = {
  t: (zh: string, en: string) => string;
  account: AccountsLedgerPilotAccount;
  closePilotActionDialog: () => void;
  credentialProbeBusy: string | null;
  activePilotManagedAccount: RouteManagedAccount | null;
  activePilotProbeResult: Pick<ConsoleCredentialProbeResult, "message" | "probePoint"> | null;
  draftDirty: boolean;
  draftMatchesActiveRevision: boolean;
  startPilotProbe: () => Promise<void>;
};

export function PilotAccountProbePanel({
  t,
  account,
  closePilotActionDialog,
  credentialProbeBusy,
  activePilotManagedAccount,
  activePilotProbeResult,
  draftDirty,
  draftMatchesActiveRevision,
  startPilotProbe,
}: PilotAccountProbePanelProps) {
  const modelTest = account.providerId === "nvidia" || activePilotManagedAccount?.providerPreset === "nvidia-openai";
  return (
    <div className="nt-stack">
      <article className="nt-pilot-dialog__hero">
        <div className="nt-pilot-dialog__hero-icon">
          <Play size={20} aria-hidden="true" />
        </div>
        <div className="nt-pilot-dialog__hero-copy">
          <strong>{account.displayName}</strong>
          <div className="nt-ledger-chip-list nt-ledger-chip-list--dense">
            <span className="nt-chip nt-chip--muted">APIKEY</span>
            <span className="nt-chip nt-chip--muted">{t("账号", "Account")}</span>
          </div>
        </div>
        <span
          className={
            account.dispatchEnabled
              ? "nt-badge nt-badge--success"
              : "nt-badge nt-badge--warning"
          }
        >
          {account.dispatchEnabled ? "active" : t("暂停", "Paused")}
        </span>
      </article>

      <div className="nt-pilot-terminal">
        {account.previewOnly
          ? t(
              "演示账号仅展示 UI，不会发起真实连接测试。",
              "Demo accounts only preview the UI and do not start a real connection test.",
            )
          : credentialProbeBusy === account.accountId
            ? modelTest
              ? t("正在使用此账号调用模型并等待回复（最长 60 秒）...", "Calling the model with this credential and waiting for a reply (up to 60 seconds)...")
              : t("正在测试连接，请稍候...", "Testing connectivity, please wait...")
            : activePilotProbeResult
              ? activePilotProbeResult.message
              : t(
                  "准备测试。点击“开始测试”按钮开始...",
                  'Ready to test. Click "Start test" to begin...',
                )}
      </div>

      <div className="nt-pilot-dialog__meta">
        <span>{modelTest
          ? t("真实模型调用测试：只有收到有效回复才通过", "Model call test: passes only after receiving a valid reply")
          : t("非生成单点测试", "Non-generative single-point test")}</span>
        <span>
          {activePilotProbeResult?.probePoint ??
            (modelTest
              ? t("使用账号配置的默认或支持模型，最多 256 个输出 token", "Uses a configured default or supported model, up to 256 output tokens")
              : t("由服务商适配器选择安全健康端点", "Safe health endpoint selected by the provider adapter"))}
        </span>
      </div>

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
          disabled={
            account.previewOnly ||
            !activePilotManagedAccount ||
            draftDirty ||
            !draftMatchesActiveRevision ||
            credentialProbeBusy === account.accountId
          }
          onClick={() => void startPilotProbe()}
        >
          {t("开始测试", "Start test")}
        </button>
      </div>
    </div>
  );
}
