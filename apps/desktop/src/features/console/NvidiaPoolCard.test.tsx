import { within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { card, pilotAccount, pilotSection, renderWorkspace, workspaceProps } from "./AccountsLedgerWorkspace.fixtures";

describe("NVIDIA pool card", () => {
  it.each([
    { kind: "Chinese", label: "很长的凭据池名称".repeat(30) },
    { kind: "unbroken Latin", label: "NVIDIA_Credential_Pool_".repeat(30) },
    { kind: "spaced Latin", label: "NVIDIA credential pool ".repeat(30).trim() },
  ])("preserves the full $kind name on both faces for visual-only truncation", ({ label }) => {
    renderWorkspace(workspaceProps({ pilotSections: [pilotSection({
      providerId: "nvidia", providerIds: ["nvidia"], providerPreset: "nvidia",
      providerLabel: label,
    })] }));
    for (const side of ["front", "back"]) {
      const heading = card("nvidia").querySelector(`.nt-provider-card__${side}-head strong`);
      expect(heading?.textContent).toBe(label);
    }
    expect(within(card("nvidia")).getByRole("button", { name: label })).toBeInTheDocument();
  });

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
