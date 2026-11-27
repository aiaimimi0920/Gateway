// No inferred off/Free defaults: missing fields remain unknown and prevent inference.
import { createHash, timingSafeEqual } from "node:crypto";
import { tokenClaims } from "./auth.mjs";
import { inspectBilling } from "./billing.mjs";
import { catalogSummary } from "./catalog_wire.mjs";
import { connectJsonHeaders, endpoints, parseJson, ProbeError, request } from "./transport.mjs";

export function emailHash(email) {
  return createHash("sha256").update(email.trim().toLowerCase()).digest("hex");
}

export function bindIdentity(token, me, expectedEmailHash) {
  const claims = tokenClaims(token);
  if (me.authId !== claims.sub || typeof me.email !== "string" || !me.email.trim() ||
      !/^[0-9a-f]{64}$/.test(expectedEmailHash) ||
      !timingSafeEqual(Buffer.from(emailHash(me.email)), Buffer.from(expectedEmailHash))) {
    throw new ProbeError("account_identity_mismatch");
  }
  const parts = claims.sub.split("|");
  const userId = parts.length === 1 ? claims.sub : parts.length === 2 ? parts[1] : "";
  if (!/^[A-Za-z0-9_.-]+$/.test(userId)) throw new ProbeError("unsupported_account_subject");
  return userId;
}

export function summarizeUsage(usage) {
  const onDemand = usage.individualUsage?.onDemand;
  const membership = typeof usage.membershipType === "string"
    ? usage.membershipType.trim().toLowerCase() : "";
  const personal = usage.limitType === "user" || usage.limitType === "individual";
  const reasons = [];
  if (membership !== "free") reasons.push("free_membership_not_confirmed");
  if (!personal) reasons.push("personal_limit_not_confirmed");
  if (onDemand?.enabled !== false) reasons.push("on_demand_not_confirmed_off");
  return {
    // Account conditions are not proof of model entitlement or of this request's billing pool.
    accountConditionsMet: reasons.length === 0, reasons,
    membership: membership === "free" ? "free" : "unconfirmed",
    limitType: personal ? usage.limitType : "unconfirmed",
    onDemandEnabled: typeof onDemand?.enabled === "boolean" ? onDemand.enabled : null,
    onDemandUsed: typeof onDemand?.used === "number" && Number.isFinite(onDemand.used)
      ? onDemand.used : null,
    includedUsed: Number.isFinite(usage.individualUsage?.plan?.used)
      ? usage.individualUsage.plan.used : null,
    includedRemaining: Number.isFinite(usage.individualUsage?.plan?.remaining)
      ? usage.individualUsage.plan.remaining : null,
    includedPercentUsed: Number.isFinite(usage.individualUsage?.plan?.totalPercentUsed)
      ? usage.individualUsage.plan.totalPercentUsed : null,
  };
}

export async function verifyIdentity(token, expectedEmailHash, options, call = request) {
  const me = parseJson(await call(endpoints.me, {
    ...options, headers: { ...connectJsonHeaders, authorization: `Bearer ${token}` }, body: "{}",
  }));
  return bindIdentity(token, me, expectedEmailHash);
}

export async function account(token, expectedEmailHash, options, call = request) {
  const userId = await verifyIdentity(token, expectedEmailHash, options, call);
  const cookie = `WorkosCursorSessionToken=${encodeURIComponent(`${userId}::${token}`)}`;
  const usage = parseJson(await call(endpoints.usage, {
    ...options, method: "GET", headers: { accept: "application/json", cookie },
  }));
  const billing = await inspectBilling(token, cookie, options, call);
  return { ...summarizeUsage(usage), accountMatched: true, billing };
}

export async function models(token, options, call = request) {
  const headers = { ...connectJsonHeaders, authorization: `Bearer ${token}` };
  const available = parseJson(await call(endpoints.models, { ...options, headers, body: "{}" }));
  const selected = parseJson(await call(endpoints.defaultModel, { ...options, headers, body: "{}" }));
  return catalogSummary(available, selected);
}
