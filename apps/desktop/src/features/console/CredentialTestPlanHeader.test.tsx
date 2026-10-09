import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { CredentialTestManager } from "./CredentialTestManager";
import { pilotSection } from "./AccountsLedgerWorkspace.fixtures";
import { defaultTestPolicy } from "./credentialTestPolicyDocument";
import { updateTestPlans, type TestPlanChange } from "./credentialTestPlansDocument";

function setup(locked = false) {
  const save = vi.fn(); const close = vi.fn(); const probe = vi.fn();
  const t = (zh: string) => zh;
  const section = pilotSection({ providerId: "p", providerIds: ["p"], providerLabel: "Pool" });
  function Harness() {
    const [document, setDocument] = useState<ConsoleRouteDocument>({ providers: [{ id: "p", label: "Pool", supported_models: ["a"],
      credential_identity_categories: [{ id: "free", label: "Free" }, { id: "plus", label: "Plus" }],
      credentials: [{ id: "one", account_name: "One", credential_identity_category_id: "free" },
        { id: "two", account_name: "Two", credential_identity_category_id: "plus" }, { id: "other", account_name: "Other" }],
      test_plans: [{ id: "existing", name: "计划一", scopes: [{ kind: "pool" }], policy: defaultTestPolicy() }],
    }], model_routes: [], aliases: {}, account_groups: [] });
    return <CredentialTestManager document={document} initialTab="auto" onProbe={probe}
      onPlanChange={(changes: TestPlanChange[]) => { const next = updateTestPlans(document, changes); save(changes); setDocument(next); return true; }}
      probe={{ t, section, closePilotActionDialog: close, providerProbeBusy: false, providerProbeError: null,
        providerProbeResponse: null, draftDirty: false, draftMatchesActiveRevision: true }}
      schedule={{ t, section, closePilotActionDialog: close, editorLocked: locked, pilotScheduleEnabled: false, setPilotScheduleEnabled: vi.fn(),
        pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: vi.fn() }} />;
  }
  render(<Harness />); return { save, close, probe };
}

it("keeps the legacy whole pool, supports subpool subsets, select all and Escape without closing the editor", async () => {
  const user = userEvent.setup(); const p = setup();
  await user.click(screen.getByRole("button", { name: "编辑" }));
  expect(screen.getByText("计划", { exact: true })).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "计划名称" })).toHaveValue("计划一");
  expect(screen.getByRole("combobox", { name: "计划类型" })).toHaveValue("subpool");
  expect(screen.queryByRole("radio")).not.toBeInTheDocument();
  const trigger = screen.getByRole("button", { name: "选择子池" });
  await user.click(trigger);
  const picker = screen.getByRole("group", { name: "选择子池" });
  expect(within(picker).getByRole("checkbox", { name: "全选" })).toBeChecked();
  await user.click(within(picker).getByRole("checkbox", { name: "Pool / Plus" }));
  expect(within(picker).getByRole("checkbox", { name: "全选" })).toBePartiallyChecked();
  await user.keyboard("{Escape}");
  expect(trigger).toHaveFocus(); expect(p.close).not.toHaveBeenCalled();
  expect(screen.getByRole("textbox", { name: "计划名称" })).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "保存计划" }));
  expect(p.save.mock.calls[0][0][0].plan.scopes).toEqual([{ kind: "subpool", id: "free" }]);
  await user.click(screen.getByRole("button", { name: "编辑" }));
  await user.click(screen.getByRole("button", { name: "选择子池" }));
  await user.click(screen.getByRole("checkbox", { name: "全选" }));
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
  await user.click(screen.getByRole("button", { name: "选择账户" }));
  await user.click(screen.getByRole("checkbox", { name: "全选" }));
  await user.click(screen.getByRole("checkbox", { name: "Other" }));
  await user.click(screen.getByRole("button", { name: "保存计划" }));
  expect(p.save.mock.calls[0][0][0].plan).toMatchObject({ scopes: [{ kind: "account", id: "one" }, { kind: "account", id: "two" }], policy: { intervalMinutes: 25 } });
  await user.click(screen.getByRole("button", { name: "编辑" }));
  expect(screen.getByRole("combobox", { name: "计划类型" })).toHaveValue("account");
  await user.click(screen.getByRole("button", { name: "选择账户" }));
  expect(screen.getByRole("checkbox", { name: "One" })).toBeChecked();
  expect(screen.getByRole("checkbox", { name: "Two" })).toBeChecked();
  expect(screen.getByRole("checkbox", { name: "Other" })).not.toBeChecked();
  await user.click(screen.getByRole("checkbox", { name: "全选" }));
  await user.click(screen.getByRole("checkbox", { name: "全选" }));
  expect(screen.getByRole("button", { name: "保存计划" })).toBeDisabled();
  await user.click(screen.getByRole("textbox", { name: "计划名称" }));
  expect(screen.queryByRole("group", { name: "选择账户" })).not.toBeInTheDocument();
  expect(p.probe).not.toHaveBeenCalled();
});
