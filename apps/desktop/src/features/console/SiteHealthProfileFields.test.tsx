import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import type { RouteAccountGroup } from "./routeAccountCatalog";
import { siteHealthConfigSchema, probePolicySchema, type SiteHealthConfig } from "./siteHealth";
import { SiteHealthProfileFields } from "./SiteHealthProfileFields";
import { SiteHealthPolicyFields } from "./SiteHealthPolicyFields";

const t = (zh: string) => zh;
const groups = (count: number): RouteAccountGroup[] => Array.from({ length: count }, (_, i) => ({
  id: `g${i + 1}`, name: `Group ${i + 1}`, description: null, billingMultiplier: 1, enabled: true, notes: null, providerCredentialIds: [],
}));
function Harness({ count, initial, onChange, disabled = false }: {
  count: number; initial: SiteHealthConfig; onChange: (config: SiteHealthConfig) => void; disabled?: boolean;
}) {
  const [config, setConfig] = useState(initial);
  return <NeuroProvider><SiteHealthProfileFields config={config} selectedId="one" groups={groups(count)} disabled={disabled}
    onChange={(next) => { setConfig(next); onChange(next); }} t={t} /></NeuroProvider>;
}
const config = (ids: string[], extra = false) => siteHealthConfigSchema.parse({
  profiles: [
    { id: "one", name: "Plan one", account_group_ids: ids, policy: {} },
    ...(extra ? [{ id: "two", name: "Plan two", account_group_ids: ["g2"], policy: {} }] : []),
  ]
});

it("keeps inline candidates and focus, protects the final binding and forbids occupied groups", async () => {
  const user = userEvent.setup(), onChange = vi.fn();
  render(<Harness count={6} initial={config(["g1"], true)} onChange={onChange} />);
  expect(screen.queryByRole("button", { name: /绑定本地凭证组/ })).not.toBeInTheDocument();
  expect(screen.getByLabelText(/Group 1/)).toBeDisabled();
  expect(screen.getByLabelText(/Group 2/)).toBeDisabled();
  const third = screen.getByLabelText(/Group 3/), name = screen.getByLabelText("方案名称");
  await user.click(third);
  await user.click(third);
  expect(screen.getByLabelText(/Group 3/)).toBe(third);
  expect(third).toHaveFocus();
  expect(name).toBe(screen.getByLabelText("方案名称"));
  expect(onChange.mock.lastCall?.[0].profiles[0].account_group_ids).toEqual(["g1"]);
  expect(onChange.mock.lastCall?.[0].profiles[1].account_group_ids).toEqual(["g2"]);
});

it("uses searchable multi-select above six candidates and retains missing bindings after unchecking", async () => {
  const user = userEvent.setup(), onChange = vi.fn();
  render(<Harness count={7} initial={config(["g1", "gone"], true)} onChange={onChange} />);
  const trigger = screen.getByRole("button", { name: /绑定本地凭证组/ });
  await user.click(trigger);
  const popup = screen.getByRole("dialog"), missing = within(popup).getByLabelText(/已删除的分组：gone/);
  await user.click(missing);
  expect(within(popup).getByLabelText(/已删除的分组：gone/)).toBe(missing);
  expect(missing).not.toBeChecked();
  expect(missing).toBeDisabled();
  expect(within(popup).getByLabelText(/Group 1/)).toBeDisabled();
  expect(within(popup).getByLabelText(/Group 2/)).toBeDisabled();
  await user.type(within(popup).getByRole("searchbox"), "not-found");
  expect(within(popup).getByRole("status")).toHaveTextContent("未找到凭证组");
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
  expect(onChange.mock.lastCall?.[0].profiles[0].account_group_ids).toEqual(["g1"]);
});

it("disables and closes an open portaled selector when editing is revoked", async () => {
  const user = userEvent.setup(), onChange = vi.fn(), initial = config(["g1"]);
  const view = render(<Harness count={7} initial={initial} onChange={onChange} />);
  const trigger = screen.getByRole("button", { name: /绑定本地凭证组/ });
  await user.click(trigger);
  expect(screen.getByRole("dialog")).toBeInTheDocument();
  view.rerender(<Harness count={7} initial={initial} onChange={onChange} disabled />);
  expect(trigger).toBeDisabled();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(onChange).not.toHaveBeenCalled();
});

it("reserves the inline group list when switching all-groups sharing", async () => {
  const user = userEvent.setup(), onChange = vi.fn();
  render(<Harness count={3} initial={config(["g1"])} onChange={onChange} />);
  const group = screen.getByRole("group", { name: /绑定本地凭证组/ });
  const second = within(group).getByLabelText(/Group 2/);
  await user.click(screen.getByLabelText("全部分组共享一次检测"));
  expect(onChange.mock.lastCall?.[0].profiles[0].account_group_ids).toEqual([]);
  expect(within(group).getByLabelText(/Group 2/)).toBe(second);
  expect(second).toBeDisabled();
  await user.click(screen.getByLabelText("全部分组共享一次检测"));
  expect(within(group).getByLabelText(/Group 2/)).toBe(second);
  expect(second).toBeEnabled();
});

it("preserves native numeric validation instead of clamping or defaulting policy input", () => {
  const onChange = vi.fn();
  render(<SiteHealthPolicyFields policy={probePolicySchema.parse({})} onChange={onChange} groups="" mapping="" onText={vi.fn()} t={t} />);
  fireEvent.change(screen.getByLabelText("刷新间隔（秒）"), { target: { value: "1" } });
  expect(onChange.mock.lastCall?.[0].refresh_interval_secs).toBe(1);
  fireEvent.change(screen.getByLabelText("刷新间隔（秒）"), { target: { value: "" } });
  expect(onChange.mock.lastCall?.[0].refresh_interval_secs).toBeNaN();
});
