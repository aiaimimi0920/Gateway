// Aggregate evidence, not inferred IQ. Unsupported/unexecuted rows never count as failed calls.
import type { ConsoleCredentialProbeResult } from "../../api/contracts";

export function summarizeTestModels(results: ConsoleCredentialProbeResult[]) {
  const models = new Map<string, { model: string; passed: number; failed: number; unsupported: number; notRun: number;
    graded: number; correct: number; highest: number; latest: string; plans: Set<string> }>();
  const latest = new Map<string, ConsoleCredentialProbeResult>();
  for (const result of results) {
    const key = JSON.stringify([result.providerId, result.credentialId, result.assessment?.planId ?? "legacy"]);
    if (!latest.has(key) || latest.get(key)!.checkedAt < result.checkedAt) latest.set(key, result);
  }
  for (const result of latest.values()) {
    for (const model of result.assessment?.models ?? []) {
      const row = models.get(model.model) ?? { model: model.model, passed: 0, failed: 0, unsupported: 0, notRun: 0,
        graded: 0, correct: 0, highest: 0, latest: "", plans: new Set<string>() };
      models.set(model.model, row);
      row.plans.add(JSON.stringify([result.providerId, result.assessment?.planId ?? result.assessment?.policySource]));
      if (result.checkedAt > row.latest) row.latest = result.checkedAt;
      for (const item of result.assessment?.cases.filter((item) => item.model === model.model) ?? []) {
        if (item.status === "passed") row.passed++;
        else if (item.status === "failed") row.failed++;
        else if (item.status === "unsupported") row.unsupported++;
        else row.notRun++;
        if (item.status === "passed" && item.correct !== null) {
          row.graded++;
          if (item.correct) { row.correct++; row.highest = Math.max(row.highest, item.difficulty); }
        }
      }
    }
  }
  return [...models.values()].sort((a, b) => a.model.localeCompare(b.model)).map((row) => {
    const attempted = row.passed + row.failed;
    const score = row.graded ? row.correct * 100 / row.graded : null;
    return { ...row, attempted, connectivity: attempted ? row.passed * 100 / attempted : null,
      errorRate: attempted ? row.failed * 100 / attempted : null,
      answerErrorRate: score === null ? null : 100 - score, score,
      level: score === null ? "unrated" : row.failed + row.unsupported + row.notRun > 0 ? "incomplete"
        : score < 80 ? "below-standard" : row.highest === 3 ? "advanced" : row.highest === 2 ? "intermediate" : "basic" };
  });
}

export const testCapabilityLevels: Record<string, [string, string]> = {
  unrated: ["未评级", "Unrated"], basic: ["L1 · 基础", "L1 · Basic"], intermediate: ["L2 · 中阶", "L2 · Intermediate"],
  advanced: ["L3 · 高阶", "L3 · Advanced"], "below-standard": ["低于测试标准", "Below test standard"], incomplete: ["证据不完整", "Incomplete evidence"],
};
