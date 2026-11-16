import { expect, it } from "vitest";
import { modelIdPolicy, validModelId } from "./providerModelId";
import { defaultTestPolicy, testScopeModelPolicy, updateTestPolicy } from "./credentialTestPolicyDocument";
import { writeProviderModelMappings } from "./providerModelMappingDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

const provider = { id: "renamed", preset: "vertex-express-api", base_url: "https://aiplatform.googleapis.com/v1/publishers/google",
  explicit_credentials_only: true, credentials: [{ id: "exact", supported_models: ["gemini-contract"] }] };
it("uses the backend's bounded single resource segment grammar without changing generic models", () => {
  const policy = modelIdPolicy(provider);
  expect(policy).toBe("vertex-express");
  expect(validModelId("A".repeat(2048), policy)).toBe(true);
  for (const id of ["A".repeat(2049), "models/foo", "x%2fsecret", "..", "x#fragment", "x:method", "中文", "x/y", "x?key=bad", "*", "-x"]) {
    expect(validModelId(id, policy), id).toBe(false);
  }
  expect(validModelId("Exact.Model/Version")).toBe(true);
  expect(modelIdPolicy({ preset: "gemini-api" })).toBe("standard");
});
it("keeps the Express grammar in account test plans and model mappings", () => {
  const document = routeDocument([provider]);
  const scope = { providerId: "renamed", kind: "account" as const, id: "exact" };
  expect(testScopeModelPolicy(document, [scope])).toBe("vertex-express");
  const plan = { ...defaultTestPolicy(), modelSelection: "selected" as const, models: ["models/foo"] };
  expect(() => updateTestPolicy(document, scope, plan)).toThrow();
  expect(() => writeProviderModelMappings({ ...provider }, [{ model: "valid-alias", upstreamModel: "models/foo" }])).toThrow();
  expect(() => updateTestPolicy(document, scope, { ...plan, models: ["gemini-contract"] })).not.toThrow();
});
