import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { CredentialTestDialog } from "./CredentialTestDialog";
import { defaultTestPolicy, testScopeKey } from "./credentialTestPolicyDocument";
import { pilotAccount, pilotSection } from "./AccountsLedgerWorkspace.fixtures";

const document: ConsoleRouteDocument = { providers: [{ id: "managed-provider", supported_models: ["a", "b"],
  test_policy: { ...defaultTestPolicy(), intervalMinutes: 30 },
  credentials: [{ id: "one", test_policy: { ...defaultTestPolicy(), intervalMinutes: 15, modelSelection: "selected", models: ["a"] } }, { id: "two" }],
}], model_routes: [], aliases: {}, account_groups: [] };
function props() {
  const t = (zh: string) => zh;
  const section = pilotSection({ directAccounts: [pilotAccount({ accountId: "one" }), pilotAccount({ accountId: "two" })] });
  return { document, initialTab: "auto" as const, onProbe: vi.fn().mockResolvedValue(undefined), onSave: vi.fn().mockReturnValue(true),
    probe: { t, section, closePilotActionDialog: vi.fn(), providerProbeBusy: false, providerProbeError: null,
      providerProbeResponse: null, draftDirty: false, draftMatchesActiveRevision: true },
    schedule: { t, section, closePilotActionDialog: vi.fn(), editorLocked: false, pilotScheduleEnabled: false,
      setPilotScheduleEnabled: vi.fn(), pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: vi.fn() } };
}

it("keeps the same form, partial models/cases and temporary input across modes and telemetry", async () => {
  const user = userEvent.setup(); const p = props(); const view = render(<CredentialTestDialog {...p} />);
  const form = screen.getByRole("group", { name: "测试计划配置" });
  await user.click(screen.getByRole("checkbox", { name: "b" }));
  await user.click(screen.getByRole("checkbox", { name: /计算能力/ }));
  await user.type(screen.getByRole("textbox", { name: "临时提示词" }), "unsent temporary prompt");
  view.rerender(<CredentialTestDialog {...p} probe={{ ...p.probe, section: { ...p.probe.section } }} />);
  expect(screen.getByRole("group", { name: "测试计划配置" })).toBe(form);
  expect(screen.getByRole("checkbox", { name: "a" })).toBeChecked();
  expect(screen.getByRole("checkbox", { name: "b" })).not.toBeChecked();
  expect(screen.getByRole("checkbox", { name: /计算能力/ })).toBeChecked();
  expect(screen.getByRole("textbox", { name: "临时提示词" })).toHaveValue("unsent temporary prompt");
  expect(p.onProbe).not.toHaveBeenCalled();
});

it("restores independent drafts after inheritance toggles and scope roundtrips", async () => {
  const user = userEvent.setup(); const p = props(); render(<CredentialTestDialog {...p} />);
  await user.click(screen.getByRole("radio", { name: "账户" }));
  const scope = screen.getByRole("combobox", { name: "选择账户" });
  const accountKey = testScopeKey({ kind: "account", providerId: "managed-provider", id: "one" });
  await user.selectOptions(scope, accountKey);
  const interval = screen.getByRole("spinbutton", { name: "执行间隔（分钟）" });
  await user.clear(interval); await user.type(interval, "17");
  await user.click(screen.getByRole("checkbox", { name: "继承" }));
  expect(interval).toHaveValue(30); expect(interval).toBeDisabled();
  await user.click(screen.getByRole("checkbox", { name: "继承" }));
  expect(interval).toHaveValue(17); expect(screen.getByRole("checkbox", { name: "a" })).toBeChecked();
  await user.click(screen.getByRole("radio", { name: "全池" }));
  await user.click(screen.getByRole("radio", { name: "账户" }));
  expect(interval).toHaveValue(17);
  expect(screen.queryByRole("button", { name: "清空账户" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "保存策略" }));
  expect(p.onSave).toHaveBeenCalledWith({ kind: "account", providerId: "managed-provider", id: "one" }, expect.objectContaining({ intervalMinutes: 17, models: ["a"] }));
});

it("does not load results or send calls inside the configuration editor", () => {
  const p = props(); const first = vi.fn().mockResolvedValue(undefined); const authorized = vi.fn().mockResolvedValue(undefined);
  const view = render(<CredentialTestDialog {...p} onLoadResults={first} />);
  expect(first).not.toHaveBeenCalled();
  view.rerender(<CredentialTestDialog {...p} onLoadResults={authorized} />);
  expect(authorized).not.toHaveBeenCalled();
  expect(p.onProbe).not.toHaveBeenCalled();
});
