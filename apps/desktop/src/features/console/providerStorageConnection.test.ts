import { buildCredentialSecretPatches } from "./credentialDocument";
import { describe, expect, it } from "vitest";
import { buildStorageConnectionPatches, readStorageConnection, storageConnectionIdentity, validateStorageConnection } from "./providerStorageConnection";

const webdav = { type: "webdav" as const, endpoint: "https://storage.example.invalid/dav", directory: "gateway", username: "fixture" };
describe("cloud storage connection validation", () => {
  it("only serializes public metadata", () => {
    expect(readStorageConnection({ ...webdav, password: "synthetic-secret", api_key: "bad" })).toEqual(webdav);
  });
  it("requires explicit HTTP acknowledgement and never accepts URL credentials", () => {
    expect(validateStorageConnection({ ...webdav, endpoint: "http://localhost:9999/dav" })).toMatch(/明文/);
    expect(validateStorageConnection({ ...webdav, endpoint: "http://localhost:9999/dav", allow_insecure_http: true })).toBeNull();
    expect(validateStorageConnection({ ...webdav, endpoint: "https://u:p@example.invalid" })).toMatch(/内嵌凭据/);
    expect(validateStorageConnection({ ...webdav, endpoint: "https://example.invalid?token=fixture" })).toMatch(/查询参数/);
  });
  it.each(["../escape", "/absolute", "a/../escape", "%2e%2e/escape", "a\\b", "a\u0000b"])("rejects unsafe prefix %s", (directory) => {
    expect(validateStorageConnection({ ...webdav, directory })).not.toBeNull();
  });
  it("requires S3 bucket and region independently from passwords", () => {
    expect(validateStorageConnection({ type: "s3", endpoint: "https://storage.example.invalid", bucket: "", region: "auto" })).toMatch(/Bucket/);
    expect(validateStorageConnection({ type: "s3", endpoint: "https://storage.example.invalid", bucket: "fixture", region: "auto" })).toBeNull();
  });
  it("supports initial connection secret patches and follows provider identity across reorder", () => {
    const document = { providers: [{ id: "other" }, { id: "p", credential_storage_connection: webdav }], model_routes: [], aliases: {} };
    const result = buildStorageConnectionPatches(document, [], [{ providerId: "p", target: "storage", connectionIdentity: storageConnectionIdentity(webdav), secrets: { password: { operation: "replace", value: "synthetic-only" } } }]);
    expect(result).toEqual([{ path: "/providers/1/credential_storage_connection/password", operation: "replace", value: "synthetic-only" }]);
    expect(JSON.stringify(document)).not.toContain("synthetic-only");
  });
  it("drops obsolete keep patches on protocol change", () => {
    const document = { providers: [{ id: "p", credential_archive_connection: { type: "local", path: "/tmp/fixture" } }], model_routes: [], aliases: {} };
    expect(buildStorageConnectionPatches(document, [{ path: "/providers/0/credential_archive_connection/password", operation: "keep" }], [])).toEqual([]);
  });
});


it("omits an unconfigured optional token when replacing S3 destination credentials", () => {
  const prior = { type: "s3" as const, endpoint: "https://first.example.invalid", bucket: "old", region: "auto" };
  const next = { ...prior, endpoint: "https://second.example.invalid", bucket: "new" };
  const activeDocument = { providers: [{ id: "p", credential_storage_connection: prior }], model_routes: [], aliases: {} };
  const draftDocument = { providers: [{ id: "p", credential_storage_connection: next }], model_routes: [], aliases: {} };
  const activeSecrets = ["access_key_id", "secret_access_key", "session_token"].map((field) => ({
    path: `/providers/0/credential_storage_connection/${field}`, configured: field !== "session_token", preview: null, fingerprint: null,
  }));
  const basePatches = buildCredentialSecretPatches({ activeDocument, draftDocument, activeSecrets });
  const patches = buildStorageConnectionPatches(draftDocument, basePatches, [{ providerId: "p", target: "storage", connectionIdentity: storageConnectionIdentity(next), secrets: {
    access_key_id: { operation: "replace", value: "new-synthetic-id" }, secret_access_key: { operation: "replace", value: "new-synthetic-secret" },
  } }]);
  expect(patches).toHaveLength(2);
  expect(patches.every((patch) => patch.operation === "replace")).toBe(true);
  expect(patches.some((patch) => patch.path.endsWith("session_token"))).toBe(false);
});
