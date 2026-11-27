// Pinned @cursor/sdk@1.4.2 DashboardService schemas, not defaults for arbitrary JSON.
// Call only after successful HTTP + JSON admission. Optional presence remains null.
import { ProbeError } from "./transport.mjs";

export function wireFields(value, names) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new ProbeError("invalid_protojson_object");
  }
  const allowed = new Map();
  for (const name of names) {
    allowed.set(name, name);
    allowed.set(name.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`), name);
  }
  const result = Object.create(null);
  for (const key of Object.keys(value)) {
    const name = allowed.get(key);
    if (!name || Object.hasOwn(result, name)) throw new ProbeError("unknown_or_duplicate_protojson_field");
    // A deliberately stricter subset: standard ProtoJSON treats null as unset.
    if (value[key] === null) throw new ProbeError("null_protojson_field");
    result[name] = value[key];
  }
  return result;
}

function scalar(value, name, kind, absent) {
  if (!Object.hasOwn(value, name)) return absent;
  const item = value[name];
  if (kind === "bool") {
    if (typeof item !== "boolean") throw new ProbeError("invalid_protojson_scalar");
    return item;
  }
  // Reject exponents, fractions and unsafe JSON numbers instead of rounding amounts.
  const integer = typeof item === "number" && Number.isSafeInteger(item) ? BigInt(item)
    : typeof item === "string" && /^-?(?:0|[1-9]\d{0,18})$/.test(item) ? BigInt(item) : null;
  const low = kind === "int32" ? -2147483648n : -9223372036854775808n;
  const high = kind === "int32" ? 2147483647n : 9223372036854775807n;
  if (integer === null || integer < low || integer > high ||
      integer < BigInt(Number.MIN_SAFE_INTEGER) || integer > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new ProbeError("invalid_protojson_integer");
  }
  return Number(integer);
}

export function creditGrants(value) {
  const fields = wireFields(value, ["hasCreditGrants", "creditBalanceCents", "totalCents", "usedCents"]);
  return {
    hasCreditGrants: scalar(fields, "hasCreditGrants", "bool", false),
    creditBalanceCents: scalar(fields, "creditBalanceCents", "int64", 0),
    totalCents: scalar(fields, "totalCents", "int64", 0),
    usedCents: scalar(fields, "usedCents", "int64", 0),
  };
}

export function teamsSummary(value) {
  const fields = wireFields(value, ["teams"]);
  const teams = Object.hasOwn(fields, "teams") ? fields.teams : [];
  if (!Array.isArray(teams) || teams.some((team) =>
    !team || typeof team !== "object" || Array.isArray(team))) {
    throw new ProbeError("invalid_team_list");
  }
  // Any team blocks admission. Never interpret role FREE_OWNER as a personal Free account.
  return { teamCount: teams.length };
}

const limitSchema = {
  hardLimit: ["int32", 0], noUsageBasedAllowed: ["bool", false],
  hardLimitPerUser: ["int32", null], perUserMonthlyLimitDollars: ["int32", 0],
  isDynamicTeamLimit: ["bool", false], hasPendingSetupOnboardingCreditClaim: ["bool", false],
  isSetupPromoActive: ["bool", false], setupPromoAmountCents: ["int32", null],
  setupPromoCreditValidityDays: ["int32", null], onDemandSpendDisabledByOrganization: ["bool", false],
  perUserFirstPartyModelsAdditionalBudgetDollars: ["int32", null],
  perUserFirstPartyModelsAdditionalBudgetUnlimited: ["bool", null],
};

export function hardLimit(value) {
  const fields = wireFields(value, Object.keys(limitSchema));
  return Object.fromEntries(Object.entries(limitSchema)
    .map(([name, [kind, absent]]) => [name, scalar(fields, name, kind, absent)]));
}

export function alternateLimitSource(limits) {
  if (!limits) return null;
  return limits.isDynamicTeamLimit || limits.hasPendingSetupOnboardingCreditClaim ||
    limits.isSetupPromoActive || limits.perUserMonthlyLimitDollars !== 0 ||
    [limits.hardLimit, limits.hardLimitPerUser, limits.setupPromoAmountCents, limits.setupPromoCreditValidityDays,
      limits.perUserFirstPartyModelsAdditionalBudgetDollars].some((value) => value !== null && value !== 0) ||
    limits.perUserFirstPartyModelsAdditionalBudgetUnlimited === true;
}
