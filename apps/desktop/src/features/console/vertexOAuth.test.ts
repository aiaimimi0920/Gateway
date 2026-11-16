import { expect, it } from "vitest";
import { isVertexOAuthProvider, normalizeVertexOAuthDraft, validVertexTarget, vertexAuthorizationTarget, vertexTargetBase } from "./vertexOAuth";
import type { CredentialDialogValue } from "./credentialDialogTypes";
import { modelIdPolicy, validModelId } from "./providerModelId";

const target = { project: "project-a", location: "global", quota_project: "quota-a" };
const provider = () => ({ id: "renamed", preset: "google-agent-platform-official-api", auth_mode: "vertex_user_oauth",
  base_url: vertexTargetBase(target), credentials: [{ id: "stable", vertex_oauth: { target, client_id: "explicit" } }] });
const value = (): CredentialDialogValue => ({ providerId: "renamed", credentialId: "stable", accountName: "Operator name", enabled: false,
  baseUrl: "", supportedModelsText: "gemini-contract", manualModelsText: "gemini-contract", fetchedModelsText: "", apiKeyOperation: "keep", apiKeyValue: "" });
it.each(["global", "us-central1", "us", "eu"])("derives the exact canonical %s target without quota headers", (location) => {
  const host = location === "global" ? "aiplatform.googleapis.com" : ["us", "eu"].includes(location) ? `aiplatform.${location}.rep.googleapis.com` : `${location}-aiplatform.googleapis.com`;
  const t = { project: "123456", location };
  expect(vertexTargetBase(t)).toBe(`https://${host}/v1/projects/123456/locations/${location}/publishers/google`);
  expect(vertexAuthorizationTarget({ ...provider(), base_url: vertexTargetBase(t) })).toEqual(t);
});
it.each([{ project: "" }, { project: "a/b" }, { project: "%2e" }, { project: "a?x" }, { project: " a" }, { project: "x".repeat(257) },
  { location: "" }, { location: "Global" }, { location: "-us" }, { location: "us-" }, { location: "us.invalid" }, { location: "x".repeat(53) },
  { quota_project: "" }, { quota_project: "quota\r\nheader" }])("rejects unsafe lexical targets: %j", (patch) => {
  expect(validVertexTarget({ ...target, ...patch })).toBe(false); expect(vertexTargetBase({ ...target, ...patch })).toBeNull();
});
it("recognizes typed product identity, never a name or generic legacy Vertex preset", () => {
  expect(isVertexOAuthProvider(provider())).toBe(true);
  for (const p of [{ ...provider(), auth_mode: undefined }, { ...provider(), preset: "vertex-express-api" }, { id: "vertex-oauth" }]) {
    expect(isVertexOAuthProvider(p)).toBe(false); expect(vertexAuthorizationTarget(p)).toBeNull();
  }
  for (const base_url of ["https://evil.invalid/v1/projects/project-a/locations/global/publishers/google", provider().base_url + "?query=x",
    "https://eu-aiplatform.googleapis.com/v1/projects/project-a/locations/eu/publishers/google"]) expect(vertexAuthorizationTarget({ ...provider(), base_url })).toBeNull();
});
it("new authorization derives only base target; stable-ID reauthorization keeps the saved quota target", () => {
  expect(vertexAuthorizationTarget(provider())).toEqual({ project: "project-a", location: "global" });
  expect(vertexAuthorizationTarget(provider(), "stable")).toEqual(target);
  expect(vertexAuthorizationTarget(provider(), "missing")).toBeNull();
  expect(vertexAuthorizationTarget({ ...provider(), credentials: [{ id: "stable", vertex_oauth: { target: { ...target, project: "other" } } }] }, "stable")).toBeNull();
});
it("allows metadata-only Keep and exact manual models without touching material", () => {
  const draft = value(); expect(normalizeVertexOAuthDraft(provider(), draft, true)).toBe(draft);
  const policy = modelIdPolicy(provider()); expect(validModelId("m".repeat(2048), policy)).toBe(true);
  for (const id of ["projects/full/models/m", "gemini%2fother", "m".repeat(2049)]) expect(validModelId(id, policy)).toBe(false);
});
it.each([{ apiKeyOperation: "replace" as const }, { apiKeyOperation: "clear" as const }, { apiKeyValue: "manual-token" },
  { baseUrl: "https://untrusted.invalid" }, { fetchedModelsText: "borrowed-model" }, { chatgptAccountId: "borrowed-account" }])("blocks material replacement or borrowed state from the metadata form: %j", (patch) => {
  expect(() => normalizeVertexOAuthDraft(provider(), { ...value(), ...patch }, true)).toThrow("OAuth");
});
it("rejects adding or retargeting through the generic editor but leaves legacy Vertex unchanged", () => {
  expect(() => normalizeVertexOAuthDraft(provider(), value(), false)).toThrow();
  const legacy = { ...provider(), auth_mode: undefined }; expect(normalizeVertexOAuthDraft(legacy, value(), false)).toEqual(value());
});
