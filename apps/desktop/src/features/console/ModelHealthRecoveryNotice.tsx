import type { ModelHealthRecovery } from "../../api/contracts/credentialTests";
import type { TranslateFn } from "./accountsLedgerTypes";

const labels: Record<ModelHealthRecovery, [string, string]> = {
  applied: ["对应模型健康状态已更新", "Exact model health updated"],
  stale_health: ["探测期间健康状态已变化，未恢复旧状态", "Health changed during the probe; stale recovery skipped"],
  stale_configuration: ["配置已变化，未应用本次健康更新", "Configuration changed; health update skipped"],
  configuration_changed_after_write: ["更新期间配置发生变化，恢复结果需重新核对", "Configuration changed during the write; recheck recovery"],
  storage_failed: ["健康状态存储未确认，请刷新核对", "Health storage not confirmed; refresh to verify"],
  unavailable: ["当前未提供持久化模型健康状态", "Persistent model health is unavailable"],
  protected: ["保留硬封禁状态，未自动恢复", "Blocked state retained; no automatic recovery"],
  failure_recorded: ["本次调用故障已记录到对应模型健康状态", "Call failure recorded for the exact model"],
  failure_not_penalized: ["本次调用失败不处罚模型路由", "This call failure does not penalize model routing"],
};

export function ModelHealthRecoveryNotice({ outcome, t }: { outcome?: ModelHealthRecovery; t: TranslateFn }) {
  if (!outcome) return null;
  return <p className="nt-copy">{t("路由健康：", "Routing health: ")}{t(...labels[outcome])}</p>;
}
