import { describe, expect, it } from "vitest";
import type { CredentialSecretEdit } from "./credentialDocument";
import { mergeCredentialSecretEditEntries } from "./secretPatchDraft";

const edit = (providerId: string, credentialId: string, value: string): CredentialSecretEdit => ({
  providerId, credentialId, field: "api_key", operation: "replace", value,
});

describe("credential secret draft merging", () => {
  it("preserves first-match ordering, distinct tuple identities and unchanged input arrays", () => {
    const first = edit("a|b", "c", "original");
    const duplicate = edit("a|b", "c", "existing-duplicate");
    const distinct = edit("a", "b|c", "distinct-identity");
    const auth: CredentialSecretEdit = {
      providerId: "a|b", credentialId: "c", field: "auth_token", operation: "clear",
    };
    const current = [first, duplicate, distinct, auth];
    const intermediate = edit("a|b", "c", "intermediate");
    const final = edit("a|b", "c", "last-edit");
    const appended = edit("__proto__", "constructor", "new-identity");
    const next = [intermediate, appended, final];
    Object.freeze(current);
    Object.freeze(next);

    const merged = mergeCredentialSecretEditEntries(current, next);

    expect(merged).toEqual([final, duplicate, distinct, auth, appended]);
    expect(merged[0]).toBe(final);
    expect(merged[1]).toBe(duplicate);
    expect(current).toEqual([first, duplicate, distinct, auth]);
    expect(next).toEqual([intermediate, appended, final]);
  });
});
