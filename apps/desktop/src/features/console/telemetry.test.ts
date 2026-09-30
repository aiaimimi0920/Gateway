import { describe, expect, it } from "vitest";
import type { ConsoleProviderCredentialModelState } from "../../api/contracts";
import {
  buildConsoleTelemetrySnapshot,
  type ConsoleTelemetryProviderAccount,
  type ConsoleTelemetrySnapshot,
} from "./telemetry";
import { credentialStatusLabel, formatRelativeSince } from "./telemetryPresentation";
import {
  rollupHasData,
  rollupProviderAccountModelTelemetry,
  rollupProviderAccountTelemetry,
} from "./telemetryRollups";

function modelState(
  overrides: Partial<ConsoleProviderCredentialModelState> = {},
): ConsoleProviderCredentialModelState {
  return {
    id: "nvidia-current-model",
    providerAccountId: "nvidia",
    providerCredentialId: null,
    providerCredentialRef: "nvidia-cred-0",
    protocolProfile: null,
    model: "nvidia/nemotron-3-super-120b-a12b",
    status: "active",
    failureClass: null,
    failureScope: null,
    failureCount: 0,
    lastError: null,
    lastUpstreamStatus: null,
    cooldownUntil: null,
    lastSuccessAt: "2026-09-27T10:11:36Z",
    lastFailureAt: null,
    updatedAt: "2026-09-27T10:11:36Z",
    ...overrides,
  };
}

function credentialHealth(states: ConsoleProviderCredentialModelState[]) {
  return buildConsoleTelemetrySnapshot({
    pressure: null,
    costOverview: null,
    requestAuditSummary: null,
    credentialModelStates: states,
    credentialInventory: null,
  }).credentials.get("nvidia-cred-0");
}

function snapshot(modelCount = 1): ConsoleTelemetrySnapshot {
  const account: ConsoleTelemetryProviderAccount = {
    providerAccountId: "account-a",
    activeConcurrency: 2,
    concurrencyLimit: 8,
    concurrencyAvailable: 6,
    breakerOpen: false,
    runtimeStatus: "active",
    requestCount: 10,
    promptTokens: 20,
    completionTokens: 30,
    totalTokens: 50,
    upstreamCostUsd: 1.25,
    lastRequestAt: "2026-09-21T01:00:00Z",
    models: [],
    modelStats: new Map(Array.from({ length: modelCount }, (_, index) => {
      const model = `model-${index}`;
      return [model, {
        model,
        requestCount: 3,
        promptTokens: 5,
        completionTokens: 7,
        totalTokens: 12,
        upstreamCostUsd: null,
        lastRequestAt: "2026-09-21T01:00:00Z",
        completedCount: 2,
        failedCount: 1,
        successWindows: [{ label: "01:00", success: 2, requests: 3 }],
      }];
    })),
    completedCount: 8,
    failedCount: 2,
    successWindows: [{ label: "01:00", success: 8, requests: 10 }],
  };
  return {
    credentials: new Map(),
    providerAccounts: new Map([["account-a", account]]),
    credentialRefsByProviderAccountId: new Map(),
    hasPressure: true,
    hasCosts: true,
    hasStates: false,
    hasAudits: true,
  };
}

describe("console telemetry ownership contracts", () => {
  const retiredModel = modelState({
    id: "nvidia-retired-model",
    model: "01-ai/yi-large",
    status: "blocked",
    failureClass: "model_unsupported",
    failureScope: "credential_model",
    failureCount: 1,
    lastError: "Configured model was not found.",
    lastUpstreamStatus: 404,
    lastSuccessAt: null,
    lastFailureAt: "2026-09-27T09:55:35Z",
    updatedAt: "2026-09-27T09:55:35Z",
  });

  it.each([false, true])("keeps a working credential usable after a different model fails (reversed: %s)", (reversed) => {
    const states = [retiredModel, modelState()];
    expect(credentialHealth(reversed ? states.reverse() : states)).toMatchObject({
      status: "active",
      failureCount: 1,
      lastError: retiredModel.lastError,
      lastFailureAt: retiredModel.lastFailureAt,
      lastSuccessAt: "2026-09-27T10:11:36Z",
    });
  });

  it("does not invent availability when every observed model is blocked", () => {
    expect(credentialHealth([retiredModel])?.status).toBe("blocked");
  });

  it.each(["credential", null])("retains account-wide or unscoped failures (%s)", (failureScope) => {
    const invalidCredential = modelState({
      ...retiredModel,
      failureClass: "credential_invalid",
      failureScope,
      lastUpstreamStatus: 401,
    });
    expect(credentialHealth([modelState(), invalidCredential])?.status).toBe("blocked");
  });

  it("counts shared provider accounts once rather than once per credential", () => {
    expect(rollupProviderAccountTelemetry(snapshot(), ["account-a", "account-a", "missing"]))
      .toEqual({
        concurrencyUsed: 2,
        concurrencyTotal: 8,
        requestCount: 10,
        upstreamCostUsd: 1.25,
        lastRequestAt: "2026-09-21T01:00:00Z",
        successWindows: [{ label: "01:00", success: 8, requests: 10 }],
      });
  });

  it("keeps unobserved provider totals distinct from observed zero traffic", () => {
    const source = snapshot();
    const absent = rollupProviderAccountTelemetry(source, ["missing"]);
    expect(absent.requestCount).toBeNull();
    expect(absent.upstreamCostUsd).toBeNull();
    expect(rollupHasData(absent)).toBe(false);
    const account = source.providerAccounts.get("account-a")!;
    account.requestCount = 0;
    expect(rollupProviderAccountTelemetry(source, ["account-a"]).requestCount).toBe(0);
    expect(rollupHasData(rollupProviderAccountTelemetry(source, ["account-a"]))).toBe(true);
  });

  it("does not attribute a shared limiter to each model or duplicate model targets", () => {
    const target = { providerAccountId: "account-a", model: "model-0" };
    expect(rollupProviderAccountModelTelemetry(snapshot(2), [target, target])).toEqual({
      concurrencyUsed: null,
      concurrencyTotal: null,
      requestCount: 3,
      upstreamCostUsd: null,
      lastRequestAt: "2026-09-21T01:00:00Z",
      successWindows: [{ label: "01:00", success: 2, requests: 3 }],
    });
    expect(rollupProviderAccountModelTelemetry(snapshot(), [target])).toMatchObject({
      concurrencyUsed: 2,
      concurrencyTotal: 8,
    });
  });

  it("distinguishes an absent poll from an empty successful state poll", () => {
    const input = {
      pressure: null,
      costOverview: null,
      requestAuditSummary: null,
      credentialModelStates: null,
      credentialInventory: null,
    };
    expect(buildConsoleTelemetrySnapshot(input).hasStates).toBe(false);
    expect(buildConsoleTelemetrySnapshot({ ...input, credentialModelStates: [] }).hasStates)
      .toBe(true);
  });

  it("preserves degraded-cooldown wording and deterministic relative time", () => {
    expect(credentialStatusLabel(" DEGRADED ", "en-US")).toBe("degraded cooldown");
    expect(credentialStatusLabel("unknown", "en-US")).toBeNull();
    expect(formatRelativeSince("2026-09-21T01:00:00Z", "en-US",
      Date.parse("2026-09-21T01:12:00Z"))).toBe("12m ago");
    expect(formatRelativeSince("invalid", "en-US", 0)).toBeNull();
  });
});
