import { within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { card, pilotAccount, pilotSection, renderWorkspace, workspaceProps } from "./AccountsLedgerWorkspace.fixtures";

describe("NVIDIA pool card", () => {
  it("shows the brand and two configured credentials before their first call", () => {
    renderWorkspace(workspaceProps({ pilotSections: [pilotSection({
      providerId: "nvidia", providerIds: ["nvidia"], providerPreset: "nvidia",
      providerLabel: "NVIDIA", poolTargetSize: 30,
      directAccounts: ["one", "two"].map((accountId) => pilotAccount({
        providerId: "nvidia", accountId, statusLabel: "待观测", requestCount: 0,
      })),
    })] }));
    const front = card("nvidia").querySelector(".nt-provider-card__front") as HTMLElement;
    expect(front.querySelector('[data-provider-icon="nvidia"] svg path')).not.toBeNull();
    expect(within(front).getByText("2/30")).toBeInTheDocument();
    expect(within(front).getByRole("img", {
      name: "可用 0，待恢复 0，失效 0，待观测 2，剩余 28",
    })).toBeInTheDocument();
    expect(front.querySelector('[data-provider-metric="requests"]')).toHaveTextContent("0");
    expect(within(front).getByText("当前统计窗口暂无调用记录")).toBeInTheDocument();
    expect(front.querySelector('[data-provider-metric="success-rate"]')).toBeNull();
  });
});
