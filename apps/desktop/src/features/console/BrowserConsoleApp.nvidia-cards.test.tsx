import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import type { ConsoleApi } from "../../api/console";
import type { ConsoleCostOverview } from "../../api/contracts";
import { createConsoleApi, preserveCommittedRouteDocument } from "./BrowserConsoleApp.api-fixture";
import { openWorkspace, providerCard, renderWithProviders, waitForCommittedRouteDraft, waitForConsoleReady } from "./BrowserConsoleApp.render-fixture";

const model = "nvidia/nemotron-3-super-120b-a12b";

async function setup(configure?: (api: ConsoleApi) => void) {
  const api = createConsoleApi();
  configure?.(api);
  const initial = await api.getRouteConfig("management-secret");
  vi.mocked(api.getRouteConfig).mockResolvedValue({ routeConfig: {
    ...initial.routeConfig,
    document: {
      providers: [{ id: "nvidia", label: "NVIDIA", preset: "nvidia-openai",
        supported_models: [model], credentials: [
          { id: "nv-a", account_name: "NVIDIA A", enabled: true },
          { id: "nv-b", account_name: "NVIDIA B", enabled: true, supported_models: [model, "deepseek-ai/deepseek-v3.2"] },
        ] }],
      model_routes: [], aliases: {}, account_groups: [],
    },
    secrets: [],
  } });
  await preserveCommittedRouteDocument(api);
  renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
  const user = userEvent.setup();
  await waitForConsoleReady();
  await openWorkspace(user, /凭据池/i);
  return { api, user };
}

describe("NVIDIA card dispatch and models", () => {
  beforeEach(() => window.localStorage.clear());

  it("persists stopping and resuming every credential in one pool click", async () => {
    const { api, user } = await setup();
    const toggle = () => within(providerCard(/^NVIDIA$/)).getByRole("switch");
    expect(toggle()).toBeChecked();
    await user.click(toggle());
    const stopped = await waitForCommittedRouteDraft(api);
    expect(stopped.providers[0]).toMatchObject({ credentials: [
      { id: "nv-a", enabled: false }, { id: "nv-b", enabled: false },
    ] });
    await waitFor(() => expect(toggle()).not.toBeChecked());
    await waitFor(() => expect(toggle()).toBeEnabled());
    const previous = vi.mocked(api.commitRouteConfig).mock.calls.length;
    await user.click(toggle());
    const resumed = await waitForCommittedRouteDraft(api, previous);
    expect(resumed.providers[0]).toMatchObject({ credentials: [
      { id: "nv-a", enabled: true }, { id: "nv-b", enabled: true },
    ] });
    await waitFor(() => expect(toggle()).toBeChecked());
    expect(vi.mocked(api.commitRouteConfig).mock.calls).toHaveLength(2);
  }, 10000);

  it("shows inherited and credential-specific models on the front faces", async () => {
    const { user } = await setup();
    const front = providerCard(/^NVIDIA$/).querySelector(".nt-provider-card__front") as HTMLElement;
    for (const summary of front.querySelectorAll(".nt-card-models summary")) await user.click(summary);
    expect(within(front).getByLabelText(model)).toHaveTextContent("nemotron-3-super-120b-a12b");
    expect(within(front).getByLabelText(model)).toBeVisible();
    expect(within(front).getByText("deepseek-v3.2")).toBeVisible();
    await user.click(screen.getByRole("button", { name: /^显示 NVIDIA 账号库$/ }));
    const a = screen.getByRole("article", { name: "NVIDIA A 账号" });
    const b = screen.getByRole("article", { name: "NVIDIA B 账号" });
    for (const card of [a, b]) {
      for (const summary of card.querySelectorAll(".nt-card-models summary")) await user.click(summary);
    }
    expect(within(a).getByLabelText(model)).toBeVisible();
    expect(within(a).queryByText("deepseek-v3.2")).not.toBeInTheDocument();
    expect(within(b).getByLabelText(model)).toBeVisible();
    expect(within(b).getByText("deepseek-v3.2")).toBeVisible();
  });

  it("renders observed pool metrics once and never turns estimated costs into income", async () => {
    const { api, user } = await setup((api) => {
      api.getRuntimePressure = vi.fn().mockResolvedValue({ pressure: {
        totalRunningRequests: 3, totalProjectConcurrency: 3, totalProviderConcurrency: 3,
        projects: [], providers: [{ providerAccountId: "nvidia", label: "NVIDIA", status: "active",
          protocolFamily: "openai", activeConcurrency: 3, concurrencyLimit: 12,
          concurrencyAvailable: 9, runningRequestCount: 3, breakerOpen: false }],
      } });
      api.getCostOverview = vi.fn().mockResolvedValue({ overview: costOverview() });
    });
    const card = providerCard(/^NVIDIA$/);
    const metric = (name: string) => card.querySelector(`[data-provider-metric="${name}"]`);
    expect(metric("concurrency")).toHaveTextContent("3/12");
    expect(metric("upstream-cost")).toHaveTextContent("$1.25");
    expect(metric("requests")).toHaveTextContent("47");
    await user.click(card.querySelector('[data-model-company="nvidia"] summary')!);
    const modelRequests = () => card.querySelector(`[data-card-model="${model}"] [data-model-metric="requests"]`);
    expect(modelRequests()).toHaveTextContent("47");
    expect(metric("platform-revenue")).toHaveTextContent("—");
    expect(within(card).queryByText("已配置 / 目标")).not.toBeInTheDocument();
    const pool = card.querySelector(".nt-provider-card__pool")!;
    expect(pool.firstElementChild).toHaveClass("nt-provider-card__pool-bar");
    expect(pool.lastElementChild).toHaveClass("nt-provider-card__pool-value");
    vi.mocked(api.getCostOverview!).mockResolvedValue({ overview: costOverview(48, 100) });
    await user.click(screen.getByRole("button", { name: /^刷新$/ }));
    await waitFor(() => expect(metric("requests")).toHaveTextContent("48"));
    expect(modelRequests()).toHaveTextContent("48");
    expect(card.querySelector('[data-model-company="nvidia"]')).toHaveAttribute("open");
    expect(metric("upstream-cost")).toHaveTextContent("≈$0.0001");
    expect(metric("platform-revenue")).toHaveTextContent("—");
  });

  it.each(["unpriced", "partial", "non-usd"])("does not present %s usage as a complete USD cost", async (kind) => {
    const overview = costOverview();
    const bucket = overview.providerBuckets[0];
    if (kind === "unpriced") bucket.estimatedMarketCostMicros = null;
    if (kind === "partial") bucket.models.push({ ...bucket.models[0], model: "unpriced-model",
      marketRate: null, estimatedMarketCostMicros: null });
    if (kind === "non-usd") bucket.models[0].marketRate!.currency = "CNY";
    await setup((api) => { api.getCostOverview = vi.fn().mockResolvedValue({ overview }); });
    const card = providerCard(/^NVIDIA$/);
    expect(card.querySelector('[data-provider-metric="upstream-cost"]')).toHaveTextContent("—");
    expect(card.querySelector('[data-provider-metric="platform-revenue"]')).toHaveTextContent("—");
    expect(card.querySelector('[data-provider-metric="requests"]')).toHaveTextContent("47");
  });
});

function costOverview(requestCount = 47, costMicros = 1250000): ConsoleCostOverview {
  return { pricingEditors: [], providerBuckets: [{
    providerAccountId: "nvidia", label: "NVIDIA", adapter: "openai", protocolFamily: "openai",
    requestCount, promptTokens: 1000, completionTokens: 2000, totalTokens: 3000,
    estimatedMarketCostMicros: costMicros, pricedModelCount: 1, unpricedModelCount: 0,
    lastRequestAt: "2026-09-27T15:00:00Z", models: [{
      model, requestCount, promptTokens: 1000, completionTokens: 2000, totalTokens: 3000,
      marketRate: { promptMicrosPer1kTokens: 250000, completionMicrosPer1kTokens: 500000,
        currency: "USD", configured: true, source: "operator" },
      estimatedMarketCostMicros: costMicros, lastRequestAt: "2026-09-27T15:00:00Z",
    }],
  }] };
}
