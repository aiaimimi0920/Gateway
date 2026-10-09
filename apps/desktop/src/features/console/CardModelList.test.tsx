import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { CardModelList } from "./CardModelList";
import { cardModelDisplayName, groupCardModels, MODEL_COMPANY_RULES } from "./modelDisplayCatalog";
import { credentialCardModelTraffic, providerCardModelTraffic } from "./cardModelTraffic";
import { buildConsoleTelemetrySnapshot } from "./telemetry";
import type { ConsoleRequestAuditProviderStats } from "../../api/contracts";

const model = "nvidia/nemotron-3-super-120b-a12b";
const displayModel = "nemotron-3-super-120b-a12b";
const t = (_zh: string, en: string) => en;

function stats(total: number, success: number): ConsoleRequestAuditProviderStats {
  const windows = [{ label: "01:00", bucketStart: "2026-09-27T01:00:00Z", totalRequests: total,
    successCount: success, failureCount: total - success }];
  const common = { totalRequests: total, completedCount: success, failedCount: total - success,
    cancelledCount: 0, runningCount: 0, lastRequestAt: null, windows };
  return { ...common, providerAccountId: "nv", models: [{ ...common, model }] };
}

describe("release-ranked callable models", () => {
  it("classifies every bare GLM prefix and declared Z.ai namespace without rewriting model IDs", () => {
    const models = ["glm-4.7-flash", "GLM-5-Turbo", "glm4.6", "glm_custom", "glm", "glm-5.1",
      "z-ai/glm-4.5-air", "Z.AI/GLM-4.6", "zai/glm-future", "custom/glm-4.7", "openai/glm-5", "not-glm-5"];
    const groups = groupCardModels(models);
    expect(groups.map((group) => group.id)).toEqual(["zai", "other"]);
    expect(groups[0].label).toBe("Z.ai");
    expect(groups[0].models).toEqual([models[5], ...models.slice(0, 5), ...models.slice(6, 9)]);
    expect(groups[1].models).toEqual(models.slice(9));
    expect(new Set(groups.flatMap((group) => group.models))).toEqual(new Set(models));
  });

  it("renders unlisted GLM models under the expandable Z.ai group", async () => {
    const { container } = render(<CardModelList models={["glm-4.7-flash", "glm4.6"]} label="fixture" t={t} />);
    expect(container.querySelector('[data-model-company="other"]')).toBeNull();
    await userEvent.setup().click(screen.getByText("Z.ai"));
    expect(screen.getByText("glm-4.7-flash")).toBeVisible();
    expect(screen.getByText("glm4.6")).toBeVisible();
  });

  it("classifies GPT and Grok variants without changing IDs, ranks or unknown namespaces", () => {
    const models = ["grok-4.7", "gpt-4.1-nano", "OPENAI/GPT-5.4-mini", "x-ai/grok-4-fast",
      "xai/grok-code-fast-1", "gpt-4o-2024-08-06", "gpt-4.1", "grok-4", "custom/gpt-4.1-nano",
      "openai/grok-4.7", "xai/gpt-4.1-nano", "openai/future-unlisted", "not-gpt-4", "gpt-", "grok-",
      "proxy/openai/gpt-4.1-nano"];
    const groups = groupCardModels(models);
    expect(groups.map((group) => group.id)).toEqual(["openai", "xai", "other"]);
    expect(groups[0].models).toEqual(["gpt-4.1", "gpt-4.1-nano", "OPENAI/GPT-5.4-mini", "gpt-4o-2024-08-06"]);
    expect(groups[1].models).toEqual(["grok-4", "grok-4.7", "x-ai/grok-4-fast", "xai/grok-code-fast-1"]);
    expect(groups[2].models).toEqual(models.slice(8));
    expect(new Set(groups.flatMap((group) => group.models))).toEqual(new Set(models));
  });

  it("renders the discovered GPT and Grok models under OpenAI and xAI, not other", async () => {
    const { container } = render(<CardModelList models={["gpt-4.1-nano", "grok-4.7"]} label="fixture" t={t} />);
    const user = userEvent.setup();
    expect(container.querySelector('[data-model-company="other"]')).toBeNull();
    await user.click(screen.getByText("OpenAI"));
    expect(screen.getByText("gpt-4.1-nano")).toBeVisible();
    await user.click(screen.getByText("xAI"));
    expect(screen.getByText("grok-4.7")).toBeVisible();
  });

  it("ranks known companies and models, never guesses unknown namespace ownership or adds capabilities", () => {
    const result = groupCardModels([model, "openai/gpt-oss-20b", "openai/future-unlisted",
      "openai/gpt-oss-120b", "local-custom", model, "  "]);
    expect(result.map((group) => group.id)).toEqual(["openai", "nvidia", "other"]);
    expect(result[0].models).toEqual(["openai/gpt-oss-120b", "openai/gpt-oss-20b"]);
    expect(result[2].models).toEqual(["openai/future-unlisted", "local-custom"]);
    expect(result[2].label).toBe("other");
    expect(result.flatMap((group) => group.models)).toHaveLength(5);
  });

  it.each([
    [model, displayModel],
    ["openai/gpt-oss-120b", "gpt-oss-120b"],
    ["QWEN/qwen3.5-397b-a17b", "qwen3.5-397b-a17b"],
    ["nvidia/nvidia-nemotron-nano-9b-v2", "nvidia-nemotron-nano-9b-v2"],
    ["gpt-oss-120b", "gpt-oss-120b"],
    ["openai/future-unlisted", "future-unlisted"],
    ["custom/model-v1", "model-v1"],
    ["/model-v1", "/model-v1"],
    ["custom/", "custom/"],
  ])("omits the organization namespace but preserves the model name in %s", (modelId, displayName) => {
    expect(cardModelDisplayName(modelId)).toBe(displayName);
  });

  it("collects all unknown models in a single final other group without guessing ownership", () => {
    const models = ["custom/model-v1", "openai/future-unlisted", "local-custom"];
    expect(groupCardModels(models)).toEqual([{ id: "other", label: "other", models }]);
  });

  it("indents model names while keeping the metric columns aligned", () => {
    const css = readFileSync(resolve(process.cwd(), "src/features/console/CardModelList.css"), "utf8");
    expect(css).toMatch(/\.nt-card-models__name\s*\{[^}]*padding-left: 40px;/);
    expect(css).toMatch(/\.nt-card-models__row\s*\{[^}]*grid-template-columns: var\(--nt-model-columns\);/);
    expect(css).toContain("--nt-model-columns: minmax(0, 1fr) 44px 54px 44px;");
    expect(css).not.toContain("--nt-color-focus-surface");
    expect(css).toMatch(/\.nt-card-models summary\s*\{[^}]*border-left: 0;/);
    expect(css).not.toContain("border-left-color");
    expect(css).toMatch(/details\[open\] > summary\s*\{[^}]*background: var\(--nt-color-control-hover\);/);
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
      credentials: [{ ...stats(1, 1), credentialRef: "a" }, { ...stats(3, 2), credentialRef: "b" }],
      retainedModelTotals: [
        { providerAccountId: "nv", credentialRef: "a", model, requestCount: 100, successCount: 90 },
        { providerAccountId: "nv", credentialRef: "b", model, requestCount: 300, successCount: 200 },
        { providerAccountId: "nv", credentialRef: null, model, requestCount: 10, successCount: 0 },
        { providerAccountId: "other", credentialRef: "a", model, requestCount: 9, successCount: 9 },
      ] };
    const snapshot = buildConsoleTelemetrySnapshot({ pressure: null, costOverview: null,
      credentialModelStates: [], credentialInventory: null, requestAuditSummary: summary });
    expect(credentialCardModelTraffic(snapshot, "nv", "a").models.get(model)).toMatchObject({ requestCount: 100, successCount: 90 });
    expect(credentialCardModelTraffic(snapshot, "nv", "b").models.get(model)).toMatchObject({ requestCount: 300, successCount: 200 });
    expect(credentialCardModelTraffic(snapshot, "another-provider", "a").models.size).toBe(0);
    expect(credentialCardModelTraffic(snapshot, "nv", "new").emptyRequestCount).toBe(0);
    const pool = providerCardModelTraffic(snapshot, ["nv", "nv"]);
    expect(pool.models.get(model)).toMatchObject({ requestCount: 410, successCount: 290 });
    expect(pool.models.get(model)?.successWindows[0].requests).toBe(4);
    const older = { ...snapshot, retainedModelTotals: undefined };
    expect(credentialCardModelTraffic(older, "nv", "a").models.get(model)?.requestCount).toBeNull();
    expect(credentialCardModelTraffic(older, "nv", "new").emptyRequestCount).toBeNull();
    expect(providerCardModelTraffic(older, ["nv"]).models.get(model)?.requestCount).toBeNull();
    expect(credentialCardModelTraffic({ ...snapshot, retainedModelTotals: [] }, "nv", "a").models.get(model))
      .toMatchObject({ requestCount: 0, successCount: 0 });
  });

  it("separates total calls, recent quality and total success without reusing the recent rate", async () => {
    const user = userEvent.setup();
    render(<CardModelList models={[model, "local-custom"]} label="pool" t={t} traffic={{
      emptyRequestCount: 0,
      models: new Map([[model, { requestCount: 100, successCount: 90,
        successWindows: [{ label: "01:00", success: 3, requests: 4 }] }]]),
    }} />);
    expect(screen.getByText(displayModel)).not.toBeVisible();
    expect(screen.getByText("local-custom")).not.toBeVisible();
    await user.click(screen.getByText("NVIDIA"));
    const name = screen.getByText(displayModel);
    const row = name.closest("li")!;
    expect(row).toBeVisible();
    expect(name).toHaveAttribute("title", model);
    expect(name).toHaveAttribute("aria-label", model);
    expect(row).toHaveAttribute("data-card-model", model);
    expect(row.querySelectorAll(":scope > [data-model-metric]")).toHaveLength(3);
    expect(within(row).getByLabelText("Total calls (retained history): 100")).toHaveTextContent("100");
    expect(within(row).getByText("90%")).toBeVisible();
    expect(within(row).queryByText("75%")).not.toBeInTheDocument();
    expect(row.querySelector('[data-model-metric="quality"]')).toHaveAttribute("role", "img");
    expect(row.querySelector('[data-model-metric="quality"]')).toHaveAttribute("title", expect.stringContaining("01:00: 3/4"));
    expect(row.querySelectorAll(".nt-card-models__cells > span")).toHaveLength(3);
    await user.click(screen.getByText("other"));
    const unknown = screen.getByText("local-custom").closest("li")!;
    expect(within(unknown).getByText("0")).toBeVisible();
    expect(within(unknown).getByText("—")).toBeVisible();
    await user.click(screen.getByText("NVIDIA"));
    expect(row).not.toBeVisible();
    await user.click(screen.getByText("other"));
    expect(unknown).not.toBeVisible();
  });

  it("keeps hours from different days distinct and orders recent quality across midnight", () => {
    const source = stats(4, 3);
    source.models[0].windows = ["2026-10-03T23:00:00Z", "2026-10-04T00:00:00Z",
      "2026-10-04T23:00:00Z"].map((bucketStart) => ({ ...source.windows[0], bucketStart,
        label: bucketStart.slice(11, 16) }));
    const snapshot = buildConsoleTelemetrySnapshot({ pressure: null, costOverview: null,
      credentialModelStates: [], credentialInventory: null, requestAuditSummary: {
        ...source, providerAccounts: [source], credentials: [{ ...source, credentialRef: "a" }],
        retainedModelTotals: [],
      } });
    for (const traffic of [providerCardModelTraffic(snapshot, ["nv"]), credentialCardModelTraffic(snapshot, "nv", "a")]) {
      expect(traffic.models.get(model)?.successWindows.map((window) => window.label))
        .toEqual(source.models[0].windows.map((window) => window.bucketStart));
    }
  });

  it("shows one-line headings and weighted company totals even while collapsed", async () => {
    const models = ["openai/gpt-oss-120b", "openai/gpt-oss-20b"];
    const user = userEvent.setup();
    const { container } = render(<CardModelList models={[...models, model]} label="pool"
      t={(zh) => zh} traffic={{ emptyRequestCount: 0, models: new Map([
        [models[0], { requestCount: 10, successCount: 10, successWindows: [{ label: "01:00", success: 2, requests: 2 }] }],
        [models[1], { requestCount: 90, successCount: 45, successWindows: [{ label: "01:00", success: 1, requests: 2 }] }],
        [model, { requestCount: 500, successCount: 500, successWindows: [] }],
      ]) }} />);
    const heading = container.querySelector(".nt-card-models__heading")!;
    expect([...heading.children].slice(1).map((element) => element.textContent)).toEqual(["调用数", "热度图", "成功率"]);
    expect(heading.querySelector("br")).toBeNull();
    const group = container.querySelector('[data-model-company="openai"]')!;
    const summary = group.querySelector("summary")!;
    expect(summary.querySelector(".nt-card-models__company-name")).toHaveTextContent("OpenAI(2)");
    expect(summary.querySelectorAll(":scope > [data-model-metric]")).toHaveLength(3);
    expect(summary.querySelector('[data-model-metric="requests"]')).toHaveTextContent("100");
    expect(summary.querySelector('[data-model-metric="success-rate"]')).toHaveTextContent("55%");
    expect(summary.querySelector('[data-model-metric="quality"]')).toHaveAttribute("title", expect.stringContaining("01:00: 3/4"));
    expect(summary).toBeVisible();
    expect(group).not.toHaveAttribute("open");
    await user.click(summary);
    expect(group).toHaveAttribute("open");
    expect(summary.querySelector('[data-model-metric="requests"]')).toHaveTextContent("100");
  });

  it("does not turn partial company telemetry into complete totals or success", () => {
    const models = ["openai/gpt-oss-120b", "openai/gpt-oss-20b"];
    const { container } = render(<CardModelList models={models} label="pool" t={t}
      traffic={{ emptyRequestCount: null, models: new Map([[models[0], {
        requestCount: 10, successCount: 8, successWindows: [],
      }]]) }} />);
    const summary = container.querySelector("summary")!;
    expect(summary.querySelector('[data-model-metric="requests"]')).toHaveTextContent("—");
    expect(summary.querySelector('[data-model-metric="success-rate"]')).toHaveTextContent("—");
  });
});
