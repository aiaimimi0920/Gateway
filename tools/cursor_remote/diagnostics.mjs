// Read-only DashboardService projections; request scopes checked against @cursor/sdk@1.4.2.
// These cross-checks are diagnostic evidence, never an alternative inference gate.
import { integerAmount } from "./billing.mjs";
import { hardLimit as decodeHardLimit } from "./account_wire.mjs";
import { connectJsonHeaders, endpoints, parseJson, ProbeError, request, settledRequests } from "./transport.mjs";

export function valueType(value) {
  if (value === undefined) return "missing";
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}

export function responseShape(value, names) {
  return {
    fieldCount: value && typeof value === "object" ? Object.keys(value).length : null,
    fields: Object.fromEntries(names.map((name) => [name, valueType(value?.[name])])),
  };
}

const bool = (value) => typeof value === "boolean" ? value : null;
const number = (value) => typeof value === "number" && Number.isFinite(value) ? value : null;

export function dashboardSummary(plan, hardLimit, period) {
  const limits = hardLimit === null ? null : decodeHardLimit(hardLimit);
  const info = plan?.planInfo;
  const name = typeof info?.planName === "string" ? info.planName.trim().toLowerCase() : "";
  const usage = period?.planUsage, spend = period?.spendLimitUsage;
  return {
    plan: {
      name: ["free", "hobby", "pro", "pro+", "ultra", "premium", "business", "enterprise"]
        .includes(name) ? name : "unconfirmed",
      includedAmountCents: integerAmount(info?.includedAmountCents),
      // This enum is the subscription store (Stripe/Apple), not personal/team ownership.
      owner: ["PLAN_OWNER_UNSPECIFIED", "PLAN_OWNER_STRIPE", "PLAN_OWNER_APPLE", 0, 1, 2]
        .includes(info?.planOwner) ? info.planOwner : null,
      shape: responseShape(plan, ["planInfo", "nextUpgrade"]),
    },
    limits: {
      noUsageBasedAllowed: bool(limits?.noUsageBasedAllowed),
      hardLimit: integerAmount(limits?.hardLimit),
      organizationDisablesOnDemand: bool(limits?.onDemandSpendDisabledByOrganization),
      setupPromoActive: bool(limits?.isSetupPromoActive),
      setupPromoAmountCents: integerAmount(limits?.setupPromoAmountCents),
      additionalBudgetDollars: integerAmount(limits?.perUserFirstPartyModelsAdditionalBudgetDollars),
      additionalBudgetUnlimited: bool(limits?.perUserFirstPartyModelsAdditionalBudgetUnlimited),
      shape: responseShape(hardLimit, ["noUsageBasedAllowed", "hardLimit", "isSetupPromoActive"]),
    },
    period: {
      includedPercentUsed: number(usage?.totalPercentUsed),
      includedRemainingCents: integerAmount(usage?.remaining),
      onDemandUsedCents: integerAmount(spend?.individualUsed),
      limitType: ["user", "individual", "team"].includes(spend?.limitType)
        ? spend.limitType : "unconfirmed",
      shape: responseShape(period, ["planUsage", "spendLimitUsage", "enabled"]),
    },
  };
}

export async function inspectDashboard(token, options, call = request) {
  const errors = [];
  const read = async (operation, endpoint, body = "{}") => {
    try {
      return parseJson(await call(endpoint, { ...options, body,
        headers: { ...connectJsonHeaders, authorization: `Bearer ${token}` } }));
    } catch (error) {
      if (options.signal?.aborted) throw new ProbeError("cancelled");
      errors.push({ operation, code: error instanceof ProbeError ? error.code : "dashboard_read_failed",
        ...(error instanceof ProbeError && error.status ? { status: error.status } : {}) });
      return null;
    }
  };
  const [plan, limits, period] = await settledRequests([
    read("plan", endpoints.plan), read("hard_limit", endpoints.hardLimit, '{"personalScope":true}'),
    read("current_period", endpoints.currentPeriod, '{"personalScope":true}'),
  ]);
  return { ...dashboardSummary(plan, limits, period), errors };
}
