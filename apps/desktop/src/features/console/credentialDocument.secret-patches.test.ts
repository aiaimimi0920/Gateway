import { describe, expect, it } from "vitest";
import { buildCredentialSecretPatches } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

describe("credential document mutations", () => {
  it("rebases keep patches by stable identities, drops deleted credentials, and adds a new API-key replacement", () => {
    const activeDocument = routeDocument([
      {
        id: "other-provider",
        api_key: "",
        credentials: [],
      },
      {
        id: "openai-main",
        api_key: "",
        credentials: [
          { id: "deleted-account", api_key: null },
          { id: "kept-account", api_key: null, auth_token: null },
        ],
      },
    ]);
    const draftDocument = routeDocument([
      {
        id: "openai-main",
        api_key: "",
        credentials: [
          { id: "new-account", api_key: null },
          { id: "kept-account", api_key: null, auth_token: null },
        ],
      },
      {
        id: "other-provider",
        api_key: "",
        credentials: [],
      },
    ]);

    const patches = buildCredentialSecretPatches({
      activeDocument,
      draftDocument,
      activeSecrets: [
        { path: "/providers/0/api_key", configured: true },
        { path: "/providers/1/api_key", configured: true },
        { path: "/providers/1/credentials/0/api_key", configured: true },
        { path: "/providers/1/credentials/1/api_key", configured: true },
        { path: "/providers/1/credentials/1/auth_token", configured: true },
      ],
      credentialSecretEdits: [
        {
          providerId: "openai-main",
          credentialId: "new-account",
          field: "api_key",
          operation: "replace",
          value: "sk-new-account",
        },
      ],
    });

    expect(patches).toEqual([
      { path: "/providers/0/api_key", operation: "keep" },
      {
        path: "/providers/0/credentials/0/api_key",
        operation: "replace",
        value: "sk-new-account",
      },
      { path: "/providers/0/credentials/1/api_key", operation: "keep" },
      { path: "/providers/0/credentials/1/auth_token", operation: "keep" },
      { path: "/providers/1/api_key", operation: "keep" },
    ]);
    expect(patches.some((patch) => patch.path.includes("deleted-account"))).toBe(false);
    expect(patches).not.toContainEqual({
      path: "/providers/0/credentials/0/api_key",
      operation: "keep",
    });
  });

  it("generates an API-key clear patch for a newly added credential", () => {
    const activeDocument = routeDocument([
      { id: "openai-main", credentials: [] },
    ]);
    const draftDocument = routeDocument([
      {
        id: "openai-main",
        credentials: [{ id: "new-account", api_key: null }],
      },
    ]);

    const patches = buildCredentialSecretPatches({
      activeDocument,
      draftDocument,
      activeSecrets: [],
      credentialSecretEdits: [
        {
          providerId: "openai-main",
          credentialId: "new-account",
          field: "api_key",
          operation: "clear",
        },
      ],
    });

    expect(patches).toEqual([
      {
        path: "/providers/0/credentials/0/api_key",
        operation: "clear",
      },
    ]);
  });

  it("preserves active secret operations while rebasing paths and lets credential edits override them", () => {
    const activeDocument = routeDocument([
      { id: "other-provider", credentials: [] },
      {
        id: "openai-main",
        auth_token: null,
        credentials: [
          { id: "account-a", api_key: null },
          { id: "account-b", api_key: null },
        ],
      },
    ]);
    const draftDocument = routeDocument([
      {
        id: "openai-main",
        auth_token: null,
        credentials: [
          { id: "account-b", api_key: null },
          { id: "account-a", api_key: null },
        ],
      },
      { id: "other-provider", credentials: [] },
    ]);

    const patches = buildCredentialSecretPatches({
      activeDocument,
      draftDocument,
      activeSecrets: [
        { path: "/providers/1/auth_token", configured: true },
        { path: "/providers/1/credentials/0/api_key", configured: true },
        { path: "/providers/1/credentials/1/api_key", configured: true },
        { path: "/global/secret", configured: true },
      ],
      activeSecretPatches: [
        { path: "/providers/1/auth_token", operation: "clear" },
        {
          path: "/providers/1/credentials/0/api_key",
          operation: "replace",
          value: "edited-account-a",
        },
        {
          path: "/providers/1/credentials/1/api_key",
          operation: "replace",
          value: "stale-account-b",
        },
        { path: "/global/secret", operation: "replace", value: "global-value" },
      ],
      credentialSecretEdits: [
        {
          providerId: "openai-main",
          credentialId: "account-b",
          field: "api_key",
          operation: "clear",
        },
      ],
    });

    expect(patches).toEqual([
      { path: "/global/secret", operation: "replace", value: "global-value" },
      { path: "/providers/0/auth_token", operation: "clear" },
      { path: "/providers/0/credentials/0/api_key", operation: "clear" },
      {
        path: "/providers/0/credentials/1/api_key",
        operation: "replace",
        value: "edited-account-a",
      },
    ]);
  });
});
