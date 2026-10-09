import type { ConsoleCredentialProbeResult } from "../../api/contracts";

// Keep account, provider, plan and test-set provenance; never average unrelated benchmarks.
export function testIqRows(results: ConsoleCredentialProbeResult[]) {
  const latest = new Map<string, ConsoleCredentialProbeResult>();
  for (const result of results) {
    const key = JSON.stringify([result.providerId, result.credentialId, result.assessment?.planId ?? "legacy"]);
    if (!latest.has(key) || latest.get(key)!.checkedAt < result.checkedAt) latest.set(key, result);
  }
  return [...latest.values()].flatMap((result) => (result.assessment?.models ?? []).map((model) => {
    const cases = result.assessment!.cases.filter((item) => item.model === model.model);
    const scored = cases.filter((item) => item.expectedAnswer !== null);
    const complete = scored.length > 0 && cases.every((item) => item.status === "passed")
      && scored.every((item) => item.correct !== null && [1, 2, 3].includes(item.difficulty));
    const weight = scored.reduce((sum, item) => sum + item.difficulty, 0);
    const correctWeight = scored.reduce((sum, item) => sum + (item.correct ? item.difficulty : 0), 0);
    const measurement = model.measurement;
    const iq = complete && measurement ? 100 * correctWeight / weight : null;
    const quota = measurement?.quota;
    const observed = quota?.status === "account-observed" && quota.unit && quota.consumed != null
      && Number.isFinite(quota.consumed) && quota.consumed > 0 ? quota.consumed : null;
    // Account deltas cannot prove a request's exact charge; ratios are explicitly observational.
    return { key: JSON.stringify([result.providerId, result.credentialId, result.assessment!.planId, model.model]),
      model: model.model, provider: result.providerId, account: result.credentialId, plan: result.assessment!.planId ?? result.assessment!.policySource,
      latest: result.checkedAt, testSetId: measurement?.testSetId, iq, complete, correct: scored.filter((item) => item.correct).length,
      scored: scored.length, attempted: cases.filter((item) => item.status === "passed" || item.status === "failed").length, total: cases.length,
      elapsedMs: measurement?.elapsedMs ?? null, quota, observed,
      quotaPerIq: iq !== null && iq > 0 && observed !== null ? observed / iq : null,
      iqPerQuota: iq !== null && observed !== null ? iq / observed : null };
  })).sort((a, b) => a.model.localeCompare(b.model) || a.key.localeCompare(b.key));
}
