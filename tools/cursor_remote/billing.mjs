// Read-only checks for alternate billing sources. Unknown or failed reads never mean zero.
import { connectJsonHeaders, endpoints, parseJson, ProbeError, request, settledRequests } from "./transport.mjs";
import { alternateLimitSource, creditGrants, hardLimit, teamsSummary } from "./account_wire.mjs";

export function integerAmount(value) {
  const number = typeof value === "number" ? value
    : typeof value === "string" && /^-?\d{1,15}$/.test(value) ? Number(value) : NaN;
  return Number.isSafeInteger(number) ? number : null;
}

export function billingSummary(grants, stripe, teams, limits) {
  return {
    hasCreditGrants: typeof grants?.hasCreditGrants === "boolean" ? grants.hasCreditGrants : null,
    creditBalanceCents: integerAmount(grants?.creditBalanceCents),
    creditGrantTotalCents: integerAmount(grants?.totalCents),
    creditGrantUsedCents: integerAmount(grants?.usedCents),
    stripeFree: typeof stripe?.membershipType === "string"
      ? stripe.membershipType.trim().toLowerCase() === "free" : null,
    customerBalance: integerAmount(stripe?.customerBalance),
    teamCount: teams?.teamCount ?? null,
    personalNoUsageBasedAllowed: limits?.noUsageBasedAllowed ?? null,
    alternateLimitSource: alternateLimitSource(limits),
  };
}

export async function inspectBilling(token, cookie, options, call = request) {
  const errors = [];
  const responseShapes = {};
  const read = async (operation, endpoint, headers, method, body, decode = (value) => value) => {
    try {
      const value = parseJson(await call(endpoint, { ...options, headers, method, body }));
      // Fixed field names only; never print unknown keys or arbitrary response values.
      const fields = operation === "credit_grants" ? ["hasCreditGrants", "creditBalanceCents", "totalCents", "usedCents",
        "data", "result", "creditGrants", "balance"] : operation === "teams" ? ["teams"] : [];
      responseShapes[operation] = { fieldCount: Object.keys(value).length,
        presentFields: fields.filter((key) => Object.hasOwn(value, key)) };
      return decode(value);
    }
    catch (error) {
      if (options.signal?.aborted) throw new ProbeError("cancelled");
      errors.push({ operation, code: error instanceof ProbeError ? error.code : "billing_read_failed",
        ...(error instanceof ProbeError && error.status ? { status: error.status } : {}) });
      return null;
    }
  };
  const bearer = { ...connectJsonHeaders, authorization: `Bearer ${token}` };
  const [grants, stripe, teams, limits] = await settledRequests([
    read("credit_grants", endpoints.grants,
      bearer, "POST", '{"includeAllProducts":true,"includePersonalTeam":true}', creditGrants),
    read("stripe", endpoints.stripe, { accept: "application/json", cookie }, "GET"),
    read("teams", endpoints.teams, bearer, "POST", '{"activeOnly":false}', teamsSummary),
    read("hard_limit", endpoints.hardLimit, bearer, "POST", '{"personalScope":true}', hardLimit),
  ]);
  return { ...billingSummary(grants, stripe, teams, limits), errors, responseShapes };
}

export function inferenceReasons(account) {
  const reasons = [];
  if (account.accountMatched !== true || account.accountConditionsMet !== true) {
    reasons.push("free_personal_account_not_confirmed");
  }
  if (account.onDemandUsed !== 0) reasons.push("zero_on_demand_usage_not_confirmed");
  // Percentages are distinct from cents; do not derive Free allowance from money fields.
  if (!Number.isFinite(account.includedPercentUsed) ||
      account.includedPercentUsed < 0 || account.includedPercentUsed >= 100) {
    reasons.push("included_allowance_not_confirmed");
  }
  const billing = account.billing;
  if (billing?.hasCreditGrants !== false || billing?.creditBalanceCents !== 0 || billing?.creditGrantTotalCents !== 0 ||
      billing?.creditGrantUsedCents !== 0) {
    reasons.push("absence_of_credit_grants_not_confirmed");
  }
  if (billing?.stripeFree !== true || billing?.customerBalance !== 0) {
    reasons.push("absence_of_paid_balance_not_confirmed");
  }
  if (billing?.teamCount !== 0) reasons.push("absence_of_team_billing_not_confirmed");
  if (billing?.personalNoUsageBasedAllowed !== true) reasons.push("personal_usage_based_not_forbidden");
  if (billing?.alternateLimitSource !== false) reasons.push("absence_of_alternate_limit_source_not_confirmed");
  if (billing?.errors?.length) reasons.push("billing_read_failed");
  return reasons;
}
