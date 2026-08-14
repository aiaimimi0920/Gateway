import { describe, expect, it } from "vitest";

import { providerVerificationFor, PROVIDER_VERIFICATION_REGISTRY } from "./providerVerificationRegistry";

describe("providerVerificationRegistry", () => {
  it("keeps live evidence secret-free and exposes verified families", () => {
    const openrouter = providerVerificationFor("openrouter");
    expect(openrouter.status).toBe("verified");
    expect(openrouter.families).toEqual(["conversation"]);
    expect(openrouter.evidenceRef).toBe("live-provider-canary-text-unique-20260812");
    const serialized = JSON.stringify(PROVIDER_VERIFICATION_REGISTRY);
    expect(serialized).not.toMatch(/Bearer\s+[A-Za-z0-9._~+/=-]+/i);
    expect(serialized).not.toMatch(/(?:api[_-]?key|cookie|secret)\s*[:=]/i);
  });

  it("does not turn failed or missing evidence into a passing account", () => {
    expect(providerVerificationFor("muyuan-openai").status).toBe("failed");
    expect(providerVerificationFor("unknown-provider").status).toBe("not-tested");
    expect(providerVerificationFor("unknown-provider").families).toEqual([]);
    expect(providerVerificationFor("lumalabs").status).toBe("blocked");
  });
});
