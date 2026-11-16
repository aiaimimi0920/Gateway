import { expect, it } from "vitest";
import { applyBedrockMaterial } from "./bedrockMaterialApply";
import { BEDROCK_FIPS_REGIONS, BEDROCK_REGIONS, awsMaterialField, bedrockEndpoint, bedrockMaterialDraft, validateBedrockMaterial, type BedrockMaterial } from "./bedrockMaterial";
import { addExplicitCredential, buildCredentialSecretPatches } from "./credentialDocument";
import { credentialDialogValueFromDocument, duplicateCredentialDialogValue } from "./providerCredentialDraft";
import { routeDocument } from "./credentialDocumentTestFixtures";

export function bedrockFixture() {
  return routeDocument([{ id: "renamed", preset: "bedrock-converse", base_url: bedrockEndpoint("us-east-1", false),
    explicit_credentials_only: true, credentials: [{ id: "a", extra_body: { AWS_ACCESS_KEY_ID: "synthetic-old", AWSSECRETACCESSKEY: "synthetic-old", AWSSESSIONTOKEN: "synthetic-old", region: "us-east-1" } }] }]);
}
export function syntheticMaterial(authMode: BedrockMaterial["authMode"] = "aws_sigv4_temporary"): BedrockMaterial {
  return { authMode, region: "us-east-1", fips: false, apiKey: authMode === "bedrock_bearer" ? "synthetic-new-bearer" : "",
    accessKeyId: authMode === "bedrock_bearer" ? "" : "synthetic-new-access", secretAccessKey: authMode === "bedrock_bearer" ? "" : "synthetic-new-secret",
    sessionToken: authMode === "aws_sigv4_temporary" ? "synthetic-new-sts" : "" };
}
const descriptors = ["AWS_ACCESS_KEY_ID", "AWSSECRETACCESSKEY", "AWSSESSIONTOKEN"].map((key) => ({ path: `/providers/0/credentials/0/extra_body/${key}`, configured: true }));

it.each(["bedrock_bearer", "aws_sigv4", "aws_sigv4_temporary"] as const)("stages %s only into secret edits and clears all old alias identities", (mode) => {
  const active = bedrockFixture();
  const redacted = structuredClone(active);
  ((redacted.providers[0] as Record<string, unknown>).credentials as Record<string, unknown>[])[0].extra_body = { region: "us-east-1" };
  const staged = applyBedrockMaterial(redacted, "renamed", "a", syntheticMaterial(mode));
  expect(JSON.stringify(staged.document)).not.toContain("synthetic-");
  staged.document.providers.unshift({ id: "other", base_url: "https://fixture.test", explicit_credentials_only: true, credentials: [] });
  const patches = buildCredentialSecretPatches({ activeDocument: active, draftDocument: staged.document, activeSecrets: descriptors, credentialSecretEdits: staged.secretEdits });
  for (const old of descriptors) expect(patches).toContainEqual({ path: old.path.replace("/providers/0/", "/providers/1/"), operation: "clear" });
  expect(patches).toContainEqual(mode === "bedrock_bearer" ? { path: "/providers/1/credentials/0/api_key", operation: "replace", value: "synthetic-new-bearer" }
    : { path: "/providers/1/credentials/0/extra_body/awsAccessKeyId", operation: "replace", value: "synthetic-new-access" });
});
it("rejects staged material after mode, audience, region, flow or refresh changes, even in hand-edited JSON", () => {
  const staged = applyBedrockMaterial(bedrockFixture(), "renamed", "a", syntheticMaterial());
  for (const change of [ { auth_mode: "aws_sigv4" }, { base_url: "https://example.test" }, { extra_body: { awsRegion: "us-west-2" } },
    { refresh_endpoint: "https://example.test/token" }, { call_flow: { call_adapter: "openai" } }, { keepalive: { service_url: "https://example.test" } } ]) {
    const draft = structuredClone(staged.document);
    Object.assign(((draft.providers[0] as Record<string, unknown>).credentials as Record<string, unknown>[])[0], change);
    const input = { activeDocument: bedrockFixture(), draftDocument: draft, activeSecrets: descriptors, credentialSecretEdits: staged.secretEdits };
    expect(() => buildCredentialSecretPatches(input)).toThrow();
    expect(buildCredentialSecretPatches({ ...input, strictSessionBindings: false }).some((p) => p.operation === "replace")).toBe(false);
  }
});
it("does not let a new account consume a provider-level shared secret through the material UI", () => {
  const active = bedrockFixture();
  const staged = applyBedrockMaterial(active, "renamed", "a", syntheticMaterial());
  expect(() => buildCredentialSecretPatches({ activeDocument: active, draftDocument: staged.document,
    activeSecrets: [{ path: "/providers/0/api_key", configured: true }], credentialSecretEdits: staged.secretEdits })).toThrow(/共享秘密/);
});
it("supports exactly the public runtime/FIPS region set and refuses conflicting or non-ASCII material", () => {
  for (const region of BEDROCK_REGIONS) expect(bedrockEndpoint(region, false)).toContain(region);
  for (const region of BEDROCK_FIPS_REGIONS) expect(bedrockEndpoint(region, true)).toContain("-fips.");
  for (const material of [{ ...syntheticMaterial(), region: "cn-north-1" }, { ...syntheticMaterial(), region: "eu-west-1", fips: true },
    { ...syntheticMaterial(), sessionToken: "" }, { ...syntheticMaterial(), secretAccessKey: "秘密" },
    { ...syntheticMaterial("bedrock_bearer"), apiKey: "Bearer synthetic" }, { ...syntheticMaterial(), apiKey: "synthetic-conflict" }]) {
    expect(() => validateBedrockMaterial(material)).toThrow();
  }
});
it("reads and duplicates only public account metadata, preserving region and exact mode but no secrets", () => {
  const input = syntheticMaterial();
  const staged = applyBedrockMaterial(bedrockFixture(), "renamed", "a", input);
  const draft = credentialDialogValueFromDocument(staged.document, "renamed", "a")!;
  expect(draft.bedrockMaterial).toMatchObject({ authMode: input.authMode, region: input.region, apiKey: "", accessKeyId: "", secretAccessKey: "", sessionToken: "" });
  expect(draft.apiKeyOperation).toBe("keep");
  const copy = duplicateCredentialDialogValue(staged.document, "renamed", { accountId: "a", enabled: true } as Parameters<typeof duplicateCredentialDialogValue>[2]);
  expect(copy.apiKeyOperation).toBe("replace");
  expect(JSON.stringify(copy)).not.toContain("synthetic-");
  expect(bedrockMaterialDraft({ preset: "bedrock-converse", base_url: "https://example.test", extra_body: { region: "us-east-1", awsRegion: "us-west-2" } }).region).toBe("");
});
it("rejects cross-product imports without mutating its source draft", () => {
  const document = addExplicitCredential(routeDocument([{ id: "api", preset: "openai", base_url: "https://api.openai.com/v1", credentials: [] }]), { providerId: "api", credential: { id: "a" } });
  const original = structuredClone(document);
  expect(() => applyBedrockMaterial(document, "api", "a", syntheticMaterial())).toThrow();
  expect(document).toEqual(original);
});
it("overrides every provider region exact key in the account without changing provider metadata", () => {
  const document = bedrockFixture();
  (document.providers[0] as Record<string, unknown>).extra_body = { region: "us-east-1", AWS_REGION: "us-east-1" };
  const original = structuredClone(document.providers[0]);
  const staged = applyBedrockMaterial(document, "renamed", "a", { ...syntheticMaterial(), region: "us-west-2" });
  const p = staged.document.providers[0] as Record<string, unknown>;
  expect(p.extra_body).toEqual((original as Record<string, unknown>).extra_body);
  expect((p.credentials as Record<string, unknown>[])[0].extra_body).toMatchObject({ region: "us-west-2", AWS_REGION: "us-west-2", awsRegion: "us-west-2" });
  expect(bedrockMaterialDraft(p, (p.credentials as Record<string, unknown>[])[0]).region).toBe("us-west-2");
});
it("recognizes only the exact AWS taxonomy, not inherited object properties", () => {
  for (const key of ["constructor", "__proto__", "toString", "awsSecretAccessKey"]) expect(awsMaterialField(key)).toBeUndefined();
});
