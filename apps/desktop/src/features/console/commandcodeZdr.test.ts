import { expect, it } from "vitest";
import { commandcodeZdrDraft, commandcodeZdrFields } from "./commandcodeZdr";
import { duplicateCredentialDialogValue, emptyCredentialDialogValue } from "./providerCredentialDraft";
import { routeDocument } from "./credentialDocumentTestFixtures";

const provider = { id: "p", preset: "commandcode-api", headers: { "x-cmd-zdr": "1" },
  credentials: [{ id: "c", headers: { "X-Cmd-Zdr": "1", unexpected: "keep-for-server-validation" } }] };
const draft = { ...emptyCredentialDialogValue("p"), credentialId: "c" };

it("preserves unspecified settings and never clears provider-level ZDR", () => {
  expect(commandcodeZdrFields(provider, draft)).toEqual({});
  expect(commandcodeZdrFields(provider, { ...draft, commandcodeZdr: "inherit" })).toEqual({ headers: { unexpected: "keep-for-server-validation" } });
  expect(provider.headers).toEqual({ "x-cmd-zdr": "1" });
  expect(commandcodeZdrDraft(provider, {})).toBe("inherit");
});

it("normalizes the account header only and ignores other products", () => {
  expect(commandcodeZdrFields(provider, { ...draft, commandcodeZdr: "required" })).toEqual({ headers: { unexpected: "keep-for-server-validation", "x-cmd-zdr": "1" } });
  expect(commandcodeZdrDraft(provider, provider.credentials[0])).toBe("required");
  expect(commandcodeZdrFields({ ...provider, preset: "openai" }, { ...draft, commandcodeZdr: "required" })).toEqual({});
  expect(commandcodeZdrDraft({ preset: "openai" }, provider.credentials[0])).toBeUndefined();
});

it("keeps the privacy requirement when duplicating an account without its secret", () => {
  const copied = duplicateCredentialDialogValue(routeDocument([provider]), "p", { accountId: "c", enabled: true } as Parameters<typeof duplicateCredentialDialogValue>[2]);
  expect(copied.commandcodeZdr).toBe("required");
  expect(copied.apiKeyValue).toBe("");
  expect(copied.apiKeyOperation).toBe("replace");
});
