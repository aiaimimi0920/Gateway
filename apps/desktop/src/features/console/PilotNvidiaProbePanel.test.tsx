import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { pilotAccount, pilotSection } from "./AccountsLedgerWorkspace.fixtures";
import { PilotAccountProbePanel } from "./PilotAccountProbePanel";
import { PilotProviderProbePanel } from "./PilotProviderProbePanel";

const t = (zh: string) => zh;

describe("NVIDIA model test explanation", () => {
  it("shows the tested model endpoint and actual reply, without claiming a health-only test", () => {
    render(<PilotAccountProbePanel t={t} account={pilotAccount({ providerId: "nvidia" })}
      activePilotManagedAccount={null} credentialProbeBusy={null} draftDirty={false}
      draftMatchesActiveRevision closePilotActionDialog={vi.fn()} startPilotProbe={vi.fn()}
      activePilotProbeResult={{ probePoint: "POST https://integrate.api.nvidia.com/v1/chat/completions (model: nvidia/test)",
        message: "Model call passed. Model: nvidia/test. Reply: OK" }} />);
    expect(screen.getByText(/只有收到有效回复才通过/)).toBeInTheDocument();
    expect(screen.getByText(/Reply: OK/)).toBeInTheDocument();
    expect(screen.getByText(/POST https/)).toBeInTheDocument();
    expect(screen.queryByText("非生成单点测试")).not.toBeInTheDocument();
  });

  it("explains the bounded generation performed by the NVIDIA bulk action", () => {
    render(<PilotProviderProbePanel t={t} section={pilotSection({ providerId: "nvidia" })}
      closePilotActionDialog={vi.fn()} providerProbeBusy={false} providerProbeError={null}
      providerProbeResponse={null} draftDirty={false} draftMatchesActiveRevision startProviderProbe={vi.fn()} />);
    expect(screen.getByText(/每个账号最多 256 个输出 token/)).toBeInTheDocument();
    expect(screen.queryByText(/不发送模型生成请求/)).not.toBeInTheDocument();
  });
});
