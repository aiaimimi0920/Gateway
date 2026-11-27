import type { ComponentProps } from "react";
import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { pilotAccount, pilotSection } from "./AccountsLedgerWorkspace.fixtures";
import { PilotActionDialog } from "./PilotActionDialog";

function fixture(kind: "probe" | "provider-probe" | "provider-schedule"): ComponentProps<typeof PilotActionDialog> {
  const providerId = "my-personal-pool";
  const account = pilotAccount({ accountId: "saved", providerId });
  const section = pilotSection({ providerId, providerIds: [providerId], directAccounts: [account] });
  return {
    t: zh => zh, pilotActionDialog: kind === "probe" ? { kind, providerId, account } : { kind, section },
    accountPilotSections: [section], testPolicyDocument: {
      providers: [{ id: providerId, auth_mode: "cursor_remote", credentials: [{ id: "saved", enabled: true }] }],
      model_routes: [], aliases: {},
    },
    closePilotActionDialog: vi.fn(), credentialProbeBusy: null, activePilotManagedAccount: null, activePilotProbeResult: null,
    draftDirty: false, draftMatchesActiveRevision: true, startPilotProbe: vi.fn(), providerProbeBusy: false, providerProbeError: null,
    providerProbeResponse: null, startProviderProbe: vi.fn(), loadProviderProbeResults: vi.fn(), applyTestPolicy: vi.fn(), editorLocked: false,
    pilotScheduleEnabled: false, setPilotScheduleEnabled: vi.fn(), pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(),
    applyProviderProbeSchedule: vi.fn(), telemetryError: null, activePilotStatsView: null, applyPilotProbeSchedule: vi.fn(),
  };
}

it.each(["probe", "provider-probe", "provider-schedule"] as const)("explains %s without asking for a prompt or calling an API", kind => {
  const props = fixture(kind);
  render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  expect(screen.getByRole("dialog", { name: "Cursor 账号测试" })).toBeVisible();
  expect(screen.getByText(/未向 Cursor 发送请求/)).toBeVisible();
  expect(screen.queryByRole("button", { name: /运行测试|保存策略|开始测试/ })).not.toBeInTheDocument();
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /^关闭$/ }));
  expect(props.closePilotActionDialog).toHaveBeenCalledOnce();
  for (const callback of [props.startPilotProbe, props.startProviderProbe, props.applyTestPolicy, props.loadProviderProbeResults]) {
    expect(callback).not.toHaveBeenCalled();
  }
});

it("keeps a mixed provider section in the normal manager", () => {
  const props = fixture("provider-schedule");
  if (props.pilotActionDialog?.kind !== "provider-schedule") throw new Error("Incorrect fixture");
  props.pilotActionDialog.section.providerIds = ["my-personal-pool", "ordinary"];
  props.testPolicyDocument!.providers.push({ id: "ordinary", credentials: [{ id: "acct" }] });
  render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  expect(screen.queryByRole("dialog", { name: "Cursor 账号测试" })).not.toBeInTheDocument();
  expect(screen.getByRole("dialog")).toBeVisible();
});
