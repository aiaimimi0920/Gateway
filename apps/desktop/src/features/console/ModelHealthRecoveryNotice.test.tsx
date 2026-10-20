import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ModelHealthRecoveryNotice } from "./ModelHealthRecoveryNotice";
import { CredentialTestResults } from "./CredentialTestResults";
import type { ConsoleProviderProbeResponse } from "../../api/contracts";
import { testScopeKey } from "./credentialTestPolicyDocument";

describe("routing health feedback", () => {
  it("keeps missing historical recovery evidence absent", () => {
    const { container } = render(<ModelHealthRecoveryNotice t={(zh) => zh} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("does not present uncertain writes as successful recovery", () => {
    render(<ModelHealthRecoveryNotice outcome="configuration_changed_after_write" t={(zh) => zh} />);
    expect(screen.getByText(/恢复结果需重新核对/)).toBeVisible();
    expect(screen.queryByText(/已更新/)).not.toBeInTheDocument();
  });

  it("shows stale routing health even when generation passed", () => {
    const response = { result: { providerId: "pool", status: "passed", message: "OK", checkedAt: "2026-10-20",
      totalCount: 1, passedCount: 1, failedCount: 0, unsupportedCount: 0, results: [
        { credentialId: "one", providerId: "pool", probePoint: "POST loopback", status: "passed", message: "OK",
          checkedAt: "2026-10-20", healthRecovery: "stale_health" },
      ] } } satisfies ConsoleProviderProbeResponse;
    render(<CredentialTestResults response={response} responses={[]} accounts={[
      { key: testScopeKey({ kind: "account", providerId: "pool", id: "one" }), id: "one", providerId: "pool", name: "One", models: ["model"], enabled: true },
    ]} busy={false} error={null} t={(zh) => zh} />);
    expect(screen.getByText(/未恢复旧状态/)).toBeVisible();
  });
});
