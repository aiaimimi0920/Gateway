// Read-only first-party DashboardService schema projection; not an entitlement gate.
// Optional bucket fields retain absence, and money/percentages are never request counts.
import { wireFields } from "./account_wire.mjs";
import { connectJsonHeaders, endpoints, parseJson, ProbeError, request, settledRequests } from "./transport.mjs";

function integer(value, optional = false) {
  if (value === undefined && optional) return null;
  if (value === undefined) return 0;
  const result = typeof value === "number" ? value
    : typeof value === "string" && /^(?:0|[1-9]\d{0,15})$/.test(value) ? Number(value) : NaN;
  if (!Number.isSafeInteger(result) || result < 0) throw new ProbeError("invalid_allowance_summary");
  return result;
}
function percent(value) {
  if (value === undefined) return null;
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    throw new ProbeError("invalid_allowance_summary");
  }
  return value;
}
function flag(value) {
  if (value === undefined) return false;
  if (typeof value !== "boolean") throw new ProbeError("invalid_allowance_summary");
  return value;
}
function standing(value) {
  const names = ["UNSPECIFIED", "PAST_DUE", "GRACE", "EXPIRING", "CANCELED", "CANCELED_IN_TRIAL"];
  const name = value === undefined ? names[0] : typeof value === "number" ? names[value]
    : typeof value === "string" ? value.replace(/^STANDING_/, "") : null;
  if (!names.includes(name)) throw new ProbeError("invalid_allowance_summary");
  return name;
}
function exhausted(used, limit, percentage) {
  // Zero/missing monetary limits do not establish a request entitlement or an exhausted pool.
  if (percentage !== null) return percentage >= 100;
  return used !== null && limit !== null && limit > 0 ? used >= limit : null;
}

export function allowanceSummary(planResponse, periodResponse, selectedModel) {
  const plan = wireFields(planResponse, ["planInfo", "nextUpgrade", "isMerged"]);
  const info = plan.planInfo === undefined ? null : wireFields(plan.planInfo, [
    "planName", "includedAmountCents", "price", "billingCycleEnd", "planOwner", "includedUsagePeriod",
    "includedUsageResetsAt", "cancelsAt", "trialEndsAt", "grokBotTrialEndsAt", "standing", "renewsAt", "restoreBy",
  ]);
  const period = wireFields(periodResponse, ["billingCycleStart", "billingCycleEnd", "planUsage",
    "spendLimitUsage", "displayThreshold", "enabled", "displayMessage", "freeBestOfNPromotion",
    "autoModelSelectedDisplayMessage", "namedModelSelectedDisplayMessage", "autoBucketModels"]);
  const usage = period.planUsage === undefined ? null : wireFields(period.planUsage, [
    "totalSpend", "includedSpend", "bonusSpend", "remaining", "limit", "remainingBonus", "bonusTooltip",
    "autoSpend", "apiSpend", "autoLimit", "apiLimit", "autoPercentUsed", "apiPercentUsed", "totalPercentUsed",
  ]);
  if (info?.planName !== undefined && typeof info.planName !== "string") {
    throw new ProbeError("invalid_allowance_summary");
  }
  const bucket = period.autoBucketModels ?? [];
  if (!Array.isArray(bucket) || bucket.some((value) => typeof value !== "string") ||
      typeof selectedModel !== "string") throw new ProbeError("invalid_allowance_summary");
  const autoUsed = integer(usage?.autoSpend, true), autoLimit = integer(usage?.autoLimit, true);
  const apiUsed = integer(usage?.apiSpend, true), apiLimit = integer(usage?.apiLimit, true);
  const autoPercent = percent(usage?.autoPercentUsed), apiPercent = percent(usage?.apiPercentUsed);
  return {
    planInfoPresent: info !== null,
    planFree: info ? ["free", "hobby"].includes(info.planName?.trim().toLowerCase()) : null,
    planStanding: info ? standing(info.standing) : null,
    usageEnabled: flag(period.enabled), planUsagePresent: usage !== null,
    totalPercentUsed: percent(usage?.totalPercentUsed),
    autoUsedCents: autoUsed, autoLimitCents: autoLimit, autoPercentUsed: autoPercent,
    apiUsedCents: apiUsed, apiLimitCents: apiLimit, apiPercentUsed: apiPercent,
    autoBucketExhausted: exhausted(autoUsed, autoLimit, autoPercent),
    apiBucketExhausted: exhausted(apiUsed, apiLimit, apiPercent),
    autoBucketModelCount: bucket.length, defaultInAutoBucket: bucket.includes(selectedModel),
    includedResetRaw: integer(info?.includedUsageResetsAt, true),
    billingCycleEndRaw: integer(period.billingCycleEnd, true),
    entitlementVerified: false,
  };
}

export async function inspectAllowance(token, selectedModel, options, call = request) {
  const headers = { ...connectJsonHeaders, authorization: `Bearer ${token}` };
  const read = async (url, body) => parseJson(await call(url, { ...options, headers, body }));
  const [plan, period] = await settledRequests([
    read(endpoints.plan, "{}"), read(endpoints.currentPeriod, '{"personalScope":true}'),
  ]);
  return allowanceSummary(plan, period, selectedModel);
}
