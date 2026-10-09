import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { CredentialTestDialog } from "./CredentialTestDialog";
import { defaultTestPolicy, testScopeKey } from "./credentialTestPolicyDocument";
import { pilotSection } from "./AccountsLedgerWorkspace.fixtures";

const document: ConsoleRouteDocument = { providers: [{ id: "p", label: "Pool", supported_models: ["a"],
  test_policy: defaultTestPolicy(), credential_identity_categories: [{ id: "free", label: "Free" }, { id: "plus", label: "Plus" }],
  credentials: [{ id: "one", account_name: "One", credential_identity_category_id: "free" },
    { id: "two", account_name: "Two", credential_identity_category_id: "plus" }],
}], model_routes: [], aliases: {}, account_groups: [] };
function props() {
  const t = (zh: string) => zh;
  const section = pilotSection({ providerId: "p", providerIds: ["p"] });
  const close = vi.fn();
  return { document, initialTab: "auto" as const, onProbe: vi.fn().mockResolvedValue(undefined), onSave: vi.fn().mockReturnValue(true),
    probe: { t, section, closePilotActionDialog: close, providerProbeBusy: false, providerProbeError: null,
      providerProbeResponse: null, draftDirty: false, draftMatchesActiveRevision: true },
    schedule: { t, section, closePilotActionDialog: close, editorLocked: false, pilotScheduleEnabled: false,
      setPilotScheduleEnabled: vi.fn(), pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: vi.fn() } };
}

it("uses three radios, reveals multiple subpools, and saves only selected targets", async () => {
  const user = userEvent.setup(); const p = props(); render(<CredentialTestDialog {...p} />);
  expect(screen.getAllByRole("radio")).toHaveLength(3);
  expect(screen.getByRole("radio", { name: "全池" })).toBeChecked();
  expect(screen.queryByRole("group", { name: "选择子池" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("radio", { name: "子池" }));
  expect(screen.getByRole("button", { name: "保存策略" })).toBeDisabled();
  const list = within(screen.getByRole("group", { name: "选择子池" }));
  await user.click(list.getByRole("checkbox", { name: /Free/ }));
  await user.click(list.getByRole("checkbox", { name: /Plus/ }));
  await user.click(screen.getByRole("checkbox", { name: "继承" }));
  const interval = screen.getByRole("spinbutton", { name: "执行间隔（分钟）" });
  await user.clear(interval); await user.type(interval, "7");
  expect(list.getAllByRole("checkbox").every((item) => (item as HTMLInputElement).checked)).toBe(true);
  await user.click(screen.getByRole("button", { name: "保存策略" }));
  expect(p.onSave).toHaveBeenCalledWith([{ kind: "subpool", providerId: "p", id: "free" }, { kind: "subpool", providerId: "p", id: "plus" }], expect.objectContaining({ intervalMinutes: 7 }));
  await user.click(screen.getByRole("radio", { name: "全池" }));
  expect(interval).toHaveValue(60);
  await user.click(screen.getByRole("radio", { name: "子池" }));
  expect(interval).toHaveValue(7);
  expect(p.onProbe).not.toHaveBeenCalled();
});

it("locks an account-card entry and cannot select another account or scope", async () => {
  const user = userEvent.setup(); const p = props();
  render(<CredentialTestDialog {...p} initialScope={{ kind: "account", providerId: "p", id: "two" }} scopeLocked />);
  expect(screen.queryByRole("radiogroup")).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "选择账户" })).not.toBeInTheDocument();
  expect(screen.getByLabelText("固定账户")).toHaveTextContent("Two");
  expect(screen.queryByRole("checkbox", { name: "One" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "全选账户" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("checkbox", { name: "继承" }));
  await user.click(screen.getByRole("button", { name: "保存策略" }));
  expect(p.onSave).toHaveBeenCalledWith({ kind: "account", providerId: "p", id: "two" }, expect.anything());
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  expect(p.onProbe).toHaveBeenCalledWith(expect.objectContaining({ scope: { kind: "account", id: "two" }, credentialIds: ["two"] }), "p");
});

it("selects one account from a pool entry and never sends calls merely by switching", async () => {
  const user = userEvent.setup(); const p = props(); render(<CredentialTestDialog {...p} />);
  await user.click(screen.getByRole("radio", { name: "账户" }));
  await user.selectOptions(screen.getByRole("combobox", { name: "选择账户" }), testScopeKey({ kind: "account", providerId: "p", id: "one" }));
  expect(screen.queryByText(/^测试账户/)).not.toBeInTheDocument();
  expect(screen.queryByRole("checkbox", { name: "Two" })).not.toBeInTheDocument();
  expect(screen.queryByText(/仅代表|不会发起模型|关闭不撤回|最多准入|自动保存成功后/)).not.toBeInTheDocument();
  expect(p.onProbe).not.toHaveBeenCalled();
});
