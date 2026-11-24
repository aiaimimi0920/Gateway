import type { ConsoleRequestAudit } from "../../api/contracts";
import type { TranslateFn } from "./operations-contracts";

const object = (value: unknown): Record<string, unknown> => value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : {};
const text = (value: unknown) => typeof value === "string" && value.trim() ? value : null;

/** Recorded failures and skipped candidates are separate facts; never guess from model names. */
export function requestOrigin(row: ConsoleRequestAudit) {
  const trace = object(row.routeTrace);
  const selected = object(trace.selectedCandidate);
  const rejected = (Array.isArray(trace.rejectedCandidates) ? trace.rejectedCandidates : []).slice(0, 100).flatMap(value => {
    const candidate = object(value);
    const provider = text(candidate.providerLabel) ?? text(candidate.providerAccountId);
    const reason = text(candidate.reason);
    return provider && reason ? [{ provider, account: text(candidate.realCredentialRef), reason }] : [];
  });
  const provider = text(selected.providerLabel) ?? text(selected.label) ?? row.providerAccountId;
  return {
    provider: provider ?? (rejected.length ? [...new Set(rejected.map(candidate => candidate.provider))].join(" / ") : null),
    account: text(trace.realCredentialRef) ?? text(selected.realCredentialRef),
    code: text(trace.errorCode),
    attempts: Array.isArray(trace.attemptedProviderIds) ? trace.attemptedProviderIds.filter((id): id is string => typeof id === "string").slice(0, 100) : [],
    noSend: trace.routeAttemptCount === 0,
    rejected,
  };
}

export function rejectionReason(reason: string, t: TranslateFn) {
  const labels: Record<string, [string, string]> = {
    upstream_balance_unavailable: ["账号余额被限制", "Account balance restricted"],
    upstream_credential_unavailable: ["账号暂不可用", "Account unavailable"],
    oauth_reauthorization_required: ["账号需要重新授权", "Account requires reauthorization"],
    site_health_route_restricted: ["站点健康策略限制", "Site health restricted"],
    model_circuit_open: ["账号模型熔断", "Account model circuit open"],
    admission_rejected: ["发送前校验失败", "Pre-send admission rejected"],
    account_disabled: ["账号已停用", "Account disabled"],
    credential_inactive: ["账号认证已停用", "Credential inactive"],
    discovery_required: ["账号尚未完成发现", "Account discovery required"],
    model_not_supported: ["账号未配置此模型", "Model not configured for account"],
    credential_auth_policy: ["账号不符合认证策略", "Account excluded by authentication policy"],
    temporarily_unavailable: ["账号被临时限制", "Account temporarily restricted"],
    credential_model_unavailable: ["账号模型被临时限制", "Account model temporarily restricted"],
  };
  return labels[reason] ? t(...labels[reason]) : reason;
}
