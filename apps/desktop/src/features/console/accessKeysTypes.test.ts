import { describe, expect, it } from "vitest";

import {
  buildAccessBundleInput,
  buildAccessKeyInput,
  buildUserCredentialInput,
  parseScopeList,
  type AccessBundleDraft,
  type AccessKeyDraft,
  type UserCredentialDraft,
} from "./accessKeysTypes";

describe("access key input contracts", () => {
  it("normalizes comma and whitespace separated scopes", () => {
    expect(parseScopeList(" chat.completions, responses\nmodels  ")).toEqual([
      "chat.completions",
      "responses",
      "models",
    ]);
  });

  it("trims key fields while preserving selected bundle identities", () => {
    const draft: AccessKeyDraft = {
      ownerType: " user ",
      ownerId: " user-1 ",
      resolvedProjectId: " project-1 ",
      resolvedTenantId: " tenant-1 ",
      keyKind: " user ",
      publicKeyPrefix: " sk-gw ",
      displayName: " Primary ",
      expiresAt: " ",
      bundleIds: ["bundle-1"],
      accountGroupIds: ["group-a", "group-b"],
      quotaMode: "unlimited",
      quotaLimit: "",
    };
    expect(buildAccessKeyInput(draft)).toEqual({
      ownerType: "user",
      ownerId: "user-1",
      resolvedProjectId: "project-1",
      resolvedTenantId: "tenant-1",
      keyKind: "user",
      publicKeyPrefix: "sk-gw",
      displayName: "Primary",
      expiresAt: null,
      bundleIds: ["bundle-1"],
      metadata: { accountGroupIds: ["group-a", "group-b"] },
      quota: { mode: "unlimited", limit: null },
    });
  });

  it("uses nullable bundle fields and a bounded credential duration fallback", () => {
    const bundle: AccessBundleDraft = {
      projectId: " ",
      slug: " starter ",
      displayName: " Starter ",
      billingMode: " ",
      status: " active ",
      description: " ",
    };
    expect(buildAccessBundleInput(bundle)).toEqual({
      projectId: null,
      slug: "starter",
      displayName: "Starter",
      billingMode: null,
      status: "active",
      description: null,
    });

    const credential: UserCredentialDraft = {
      userId: " user-1 ",
      projectId: " ",
      credentialType: " session ",
      durationDays: "-1",
      scope: " chat.completions, responses ",
    };
    expect(buildUserCredentialInput(credential)).toEqual({
      userId: "user-1",
      projectId: null,
      credentialType: "session",
      durationDays: 30,
      scope: ["chat.completions", "responses"],
    });
  });
});
