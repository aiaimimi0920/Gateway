import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { SiteHealthAdapterFields } from "./SiteHealthAdapterFields";
import { gatewayHealthFamily } from "./gatewayHealthAdapters";
import { applySiteHealth, probePolicySchema, siteHealthConfigSchema, type ProbePolicy } from "./siteHealth";
import { validateSiteHealthPlans } from "./siteHealthValidation";
import { healthSecretAudience } from "./siteHealthSecretDraft";

afterEach(cleanup);
const t = (zh: string) => zh;
const legacy = ["new-api-perf-summary", "model-status-all", "channel-health", "model-status-embed", "model-health-slot", "sub2api-v2"] as const;

it("offers gateway projects and official services, never internal reader variants", () => {
  const onChange = vi.fn();
  render(<SiteHealthAdapterFields policy={probePolicySchema.parse({})} onChange={onChange} t={t} />);
  const select = screen.getByLabelText("适配器") as HTMLSelectElement;
  const values = Array.from(select.options, (option) => option.value);
  expect(values).toEqual(expect.arrayContaining(["new-api", "one-api", "sub2api", "compatible-gateway", "openai-status", "anthropic-status"]));
  for (const id of legacy) expect(values).not.toContain(id);
  fireEvent.change(select, { target: { value: "new-api" } });
  expect(onChange.mock.calls[0][0].adapter).toBe("new-api");
  fireEvent.change(select, { target: { value: "one-api" } });
  expect(onChange.mock.calls[1][0].adapter).toBe("one-api");
});

it.each(legacy)("displays legacy %s as a family without rewriting saved policy or auth scope", (adapter) => {
  const policy = probePolicySchema.parse({ adapter, auth: { kind: "bearer" } });
  const before = structuredClone(policy);
  const onChange = vi.fn();
  render(<SiteHealthAdapterFields policy={policy} onChange={onChange} t={t} />);
  expect(screen.getByLabelText("适配器")).toHaveValue(gatewayHealthFamily(adapter));
  expect(onChange).not.toHaveBeenCalled();
  expect(policy).toEqual(before);
  const config = siteHealthConfigSchema.parse({ ...policy, profiles: [{ id: "plan", name: "Plan", policy }] });
  const document = applySiteHealth({ providers: [{ id: "site" }], model_routes: [], aliases: {} }, "site", config);
  expect((document.providers[0] as any).site_health.adapter).toBe(adapter);
  expect((document.providers[0] as any).site_health.profiles[0].policy.adapter).toBe(adapter);
  fireEvent.click(screen.getByRole("button", { name: "改为按网关自动识别" }));
  const changed: ProbePolicy = onChange.mock.calls[0][0];
  expect(changed.adapter).toBe(gatewayHealthFamily(adapter));
  expect(changed.auth).toEqual(before.auth);
  expect(healthSecretAudience({ base_url: "https://site.invalid" }, changed)).not.toBe(healthSecretAudience({ base_url: "https://site.invalid" }, before));
});

it.each(["sub2api", "sub2api-v2"])("validates numeric remote groups for %s", (adapter) => {
  expect(() => validateSiteHealthPlans(siteHealthConfigSchema.parse({ adapter, monitor_groups: ["free"] }), new Set())).toThrow("正整数");
  expect(() => validateSiteHealthPlans(siteHealthConfigSchema.parse({ adapter, monitor_groups: ["12"] }), new Set())).not.toThrow();
});

it("does not claim extension-only legacy schemas establish a New API or One API identity", () => {
  for (const adapter of legacy.slice(1, 5)) expect(gatewayHealthFamily(adapter)).toBe("compatible-gateway");
  render(<SiteHealthAdapterFields policy={probePolicySchema.parse({ adapter: "one-api" })} onChange={vi.fn()} t={t} />);
  expect(screen.getByRole("note")).toHaveTextContent("不保证原版提供");
});
