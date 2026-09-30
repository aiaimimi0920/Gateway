import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { CardModelList } from "./CardModelList";
import { groupCardModels, MODEL_COMPANY_RULES } from "./modelDisplayCatalog";
import { credentialCardModelTraffic, providerCardModelTraffic } from "./cardModelTraffic";
import { buildConsoleTelemetrySnapshot } from "./telemetry";
import type { ConsoleRequestAuditProviderStats } from "../../api/contracts";

const model = "nvidia/nemotron-3-super-120b-a12b";
const t = (_zh: string, en: string) => en;

function stats(total: number, success: number): ConsoleRequestAuditProviderStats {
  const windows = [{ label: "01:00", bucketStart: "2026-09-27T01:00:00Z", totalRequests: total,
    successCount: success, failureCount: total - success }];
  const common = { totalRequests: total, completedCount: success, failedCount: total - success,
    cancelledCount: 0, runningCount: 0, lastRequestAt: null, windows };
  return { ...common, providerAccountId: "nv", models: [{ ...common, model }] };
}

describe("release-ranked callable models", () => {
  it("ranks known companies and models, never guesses unknown namespace ownership or adds capabilities", () => {
    const result = groupCardModels([model, "openai/gpt-oss-20b", "openai/future-unlisted",
      "openai/gpt-oss-120b", "local-custom", model, "  "]);
    expect(result.map((group) => group.id)).toEqual(["openai", "nvidia", "unclassified"]);
    expect(result[0].models).toEqual(["openai/gpt-oss-120b", "openai/gpt-oss-20b"]);
    expect(result[2].models).toEqual(["openai/future-unlisted", "local-custom"]);
    expect(result.flatMap((group) => group.models)).toHaveLength(5);
  });

  it("has no ambiguous alias or company entries in the release catalogue", () => {
    const companies = MODEL_COMPANY_RULES.map((company) => company.id);
    const aliases = MODEL_COMPANY_RULES.flatMap((company) => company.models.flatMap((name) =>
      [name, ...company.namespaces.map((namespace) => `${namespace}/${name}`)]));
    expect(new Set(companies).size).toBe(companies.length);
    expect(new Set(aliases.map((alias) => alias.toLowerCase())).size).toBe(aliases.length);
  });

  it("keeps credential/model attribution, unknown telemetry and zero observed traffic distinct", () => {
    const summary = { ...stats(4, 3), providerAccounts: [stats(4, 3)],
      credentials: [{ ...stats(1, 1), credentialRef: "a" }, { ...stats(3, 2), credentialRef: "b" }] };
    const snapshot = buildConsoleTelemetrySnapshot({ pressure: null, costOverview: null,
      credentialModelStates: [], credentialInventory: null, requestAuditSummary: summary });
    expect(credentialCardModelTraffic(snapshot, "nv", "a").models.get(model)?.requestCount).toBe(1);
    expect(credentialCardModelTraffic(snapshot, "nv", "b").models.get(model)?.requestCount).toBe(3);
    expect(credentialCardModelTraffic(snapshot, "another-provider", "a").models.size).toBe(0);
    expect(credentialCardModelTraffic(snapshot, "nv", "new").emptyRequestCount).toBe(0);
    const pool = providerCardModelTraffic(snapshot, ["nv", "nv"]);
    expect(pool.models.get(model)?.requestCount).toBeNull();
    expect(pool.models.get(model)?.successWindows[0].requests).toBe(4);
    expect(credentialCardModelTraffic({ ...snapshot, credentialAuditStats: undefined }, "nv", "a")
      .emptyRequestCount).toBeNull();
  });

  it("expands a company, displays a real per-model rate and keeps unobserved rates unavailable", async () => {
    const user = userEvent.setup();
    render(<CardModelList models={[model, "local-custom"]} label="pool" t={t} traffic={{
      countScope: "recent", emptyRequestCount: 0,
      models: new Map([[model, { requestCount: 4, successWindows: [{ label: "01:00", success: 3, requests: 4 }] }]]),
    }} />);
    expect(screen.getByText(model)).not.toBeVisible();
    await user.click(screen.getByText("NVIDIA"));
    const row = screen.getByText(model).closest("li")!;
    expect(row).toBeVisible();
    expect(within(row).getByLabelText("Recent calls in the current audit sample: 4")).toHaveTextContent("4");
    expect(within(row).getByText("75%")).toBeVisible();
    const unknown = screen.getByText("local-custom").closest("li")!;
    expect(within(unknown).getByText("0")).toBeVisible();
    expect(within(unknown).getByText("—")).toBeVisible();
    await user.click(screen.getByText("NVIDIA"));
    expect(row).not.toBeVisible();
  });
});
