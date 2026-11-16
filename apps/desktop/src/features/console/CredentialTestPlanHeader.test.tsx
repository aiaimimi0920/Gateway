import { render as renderBase, screen, within } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { CredentialTestManager } from "./CredentialTestManager";
import { pilotSection } from "./AccountsLedgerWorkspace.fixtures";
import { defaultTestPolicy } from "./credentialTestPolicyDocument";
import { updateTestPlans, type TestPlanChange } from "./credentialTestPlansDocument";
const render = (ui: Parameters<typeof renderBase>[0]) => renderBase(ui, { wrapper: NeuroProvider });

function setup(locked = false, many = false) {
  const save = vi.fn(); const close = vi.fn(); const probe = vi.fn();
  const t = (zh: string) => zh;
  const section = pilotSection({ providerId: "p", providerIds: ["p"], providerLabel: "Pool" });
  function Harness() {
    const [document, setDocument] = useState<ConsoleRouteDocument>({
      providers: [{
        id: "p", label: "Pool", supported_models: ["a"],
        credential_identity_categories: [{ id: "free", label: "Free" }, { id: "plus", label: "Plus" }, ...(many ? Array.from({ length: 6 }, (_, i) => ({ id: `s${i}`, label: `Group ${i}` })) : [])],
        credentials: [{ id: "one", account_name: "One", credential_identity_category_id: "free" },
        { id: "two", account_name: "Two", credential_identity_category_id: "plus" }, { id: "other", account_name: "Other" }],
        test_plans: [{ id: "existing", name: "计划一", scopes: [{ kind: "pool" }], policy: defaultTestPolicy() }],
      }], model_routes: [], aliases: {}, account_groups: []
    });
    return <CredentialTestManager document={document} initialTab="auto" onProbe={probe}
      onPlanChange={(changes: TestPlanChange[]) => { const next = updateTestPlans(document, changes); save(changes); setDocument(next); return true; }}
      probe={{
        t, section, closePilotActionDialog: close, providerProbeBusy: false, providerProbeError: null,
        providerProbeResponse: null, draftDirty: false, draftMatchesActiveRevision: true
      }}
      schedule={{
        t, section, closePilotActionDialog: close, editorLocked: locked, pilotScheduleEnabled: false, setPilotScheduleEnabled: vi.fn(),
        pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: vi.fn()
      }} />;
  }
  render(<Harness />); return { save, close, probe };
}

it("keeps whole-pool membership distinct from a subset without remounting the inline choices", async () => {
  const user = userEvent.setup(); const p = setup();
  await user.click(screen.getByRole("button", { name: "编辑" }));
  expect(screen.getByText("计划", { exact: true })).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "计划名称" })).toHaveValue("计划一");
  expect(screen.getByRole("combobox", { name: "计划类型" })).toHaveValue("subpool");
  expect(screen.queryByRole("radio")).not.toBeInTheDocument();
  const picker = screen.getByRole("group", { name: "选择子池" });
  const all = screen.getByRole("checkbox", { name: "整个凭据池（含未分组及新增账户）" });
  const plus = within(picker).getByRole("checkbox", { name: "Pool / Plus" });
  expect(all).toBeChecked();
  await user.click(plus);
  expect(all).toBePartiallyChecked();
  expect(within(picker).getByRole("checkbox", { name: "Pool / Plus" })).toBe(plus);
  expect(plus).toHaveFocus(); expect(p.close).not.toHaveBeenCalled();
  expect(screen.getByRole("textbox", { name: "计划名称" })).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "保存计划" }));
  expect(p.save.mock.calls[0][0][0].plan.scopes).toEqual([{ kind: "subpool", id: "free" }]);
  await user.click(screen.getByRole("button", { name: "编辑" }));
  await user.click(screen.getByRole("checkbox", { name: "整个凭据池（含未分组及新增账户）" }));
  await user.click(screen.getByRole("button", { name: "保存计划" }));
  expect(p.save.mock.calls[1][0][0].plan.scopes).toEqual([{ kind: "pool" }]);
  expect(p.probe).not.toHaveBeenCalled();
});

it("saves multiple accounts, roundtrips the selection and preserves edited policy when changing targets", async () => {
  const user = userEvent.setup(); const p = setup();
  await user.click(screen.getByRole("button", { name: "编辑" }));
  await user.clear(screen.getByRole("spinbutton", { name: "执行间隔（分钟）" }));
  await user.type(screen.getByRole("spinbutton", { name: "执行间隔（分钟）" }), "25");
  await user.selectOptions(screen.getByRole("combobox", { name: "计划类型" }), "account");
  expect(screen.getByRole("button", { name: "保存计划" })).toBeDisabled();
  await user.click(screen.getByRole("checkbox", { name: "全选账户" }));
  await user.click(screen.getByRole("checkbox", { name: "Other" }));
  await user.click(screen.getByRole("button", { name: "保存计划" }));
  expect(p.save.mock.calls[0][0][0].plan).toMatchObject({ scopes: [{ kind: "account", id: "one" }, { kind: "account", id: "two" }], policy: { intervalMinutes: 25 } });
  await user.click(screen.getByRole("button", { name: "编辑" }));
  expect(screen.getByRole("combobox", { name: "计划类型" })).toHaveValue("account");
  expect(screen.getByRole("checkbox", { name: "One" })).toBeChecked();
  expect(screen.getByRole("checkbox", { name: "Two" })).toBeChecked();
  expect(screen.getByRole("checkbox", { name: "Other" })).not.toBeChecked();
  await user.click(screen.getByRole("checkbox", { name: "全选账户" }));
  await user.click(screen.getByRole("checkbox", { name: "全选账户" }));
  expect(screen.getByRole("button", { name: "保存计划" })).toBeDisabled();
  await user.click(screen.getByRole("textbox", { name: "计划名称" }));
  expect(screen.getByRole("group", { name: "选择账户" })).toBeInTheDocument();
  expect(p.probe).not.toHaveBeenCalled();
});

it("uses a searchable dropdown for many targets and Escape closes only that dropdown", async () => {
  const user = userEvent.setup(); const p = setup(false, true);
  await user.click(screen.getByRole("button", { name: "编辑" }));
  const trigger = screen.getByRole("button", { name: /^选择子池，/ });
  await user.click(trigger);
  const picker = screen.getByRole("group", { name: "选择子池" });
  const plus = within(picker).getByRole("checkbox", { name: "Pool / Plus" });
  await user.click(plus);
  expect(within(picker).getByRole("checkbox", { name: "Pool / Plus" })).toBe(plus);
  expect(plus).toHaveFocus();
  await user.keyboard("{Escape}");
  expect(trigger).toHaveFocus(); expect(p.close).not.toHaveBeenCalled();
  expect(screen.getByRole("textbox", { name: "计划名称" })).toBeInTheDocument();
});

it("does not label every listed subpool as whole-pool membership", async () => {
  const user = userEvent.setup(); const p = setup();
  await user.click(screen.getByRole("button", { name: "编辑" }));
  const plus = screen.getByRole("checkbox", { name: "Pool / Plus" });
  await user.click(plus); await user.click(plus);
  expect(screen.getByRole("checkbox", { name: "整个凭据池（含未分组及新增账户）" })).not.toBeChecked();
  await user.click(screen.getByRole("button", { name: "保存计划" }));
  expect(p.save.mock.calls[0][0][0].plan.scopes).toEqual([{ kind: "subpool", id: "free" }, { kind: "subpool", id: "plus" }]);
});
