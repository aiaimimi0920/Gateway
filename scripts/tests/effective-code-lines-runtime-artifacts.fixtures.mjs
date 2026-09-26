// Synthetic audit receipts exercise approval/hash binding, not real signatures.
import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const sourceRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const templatePolicy = JSON.parse(fs.readFileSync(path.join(sourceRoot, "scripts/effective-code-lines-policy.json"), "utf8"));
const hash = (value) => crypto.createHash("sha256").update(value).digest("hex");

export function makeArtifactFixture(context) {
  const temporaryRoot = fs.realpathSync(os.tmpdir());
  const root = fs.mkdtempSync(path.join(temporaryRoot, "gateway-artifact-gate-"));
  context.after(() => {
    assert.equal(path.dirname(fs.realpathSync(root)), temporaryRoot);
    fs.rmSync(root, { recursive: true });
  });
  function write(file, content) {
    const target = path.join(root, ...file.split("/"));
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, content);
    return target;
  }
  const itemId = "a".repeat(32);
  const version = "1.2.3";
  const packagePath = `deploy/gateway_data/browser-profiles/suno/Default/Extensions/${itemId}/${version}_0`;
  const artifactPath = `${packagePath}/runtime.js`;
  const content = Array.from({ length: 800 }, (_, index) => `const item${index} = ${index};`).join("\n") + "\n";
  const file = { path: "runtime.js", bytes: Buffer.byteLength(content), sha256: hash(content),
    signedTreeHash: "a".repeat(43), role: "strict-inventory-asset", byteMutationRejected: true };
  const receipt = {
    schemaVersion: 1, evidenceType: "chromium-webstore-rs256-treehash",
    publicKeySha256: "b5e4096227b83f82101fc2aa49f2a251accde3d98471464dd17d1819b43786b0",
    chromiumCommit: "a".repeat(40),
    packages: [{ path: packagePath, itemId, version, verifiedContentsSha256: "b".repeat(64),
      webstoreSignatureVerified: true, signatureMutationRejected: true, files: [file] }],
  };
  const registry = {
    schemaVersion: 1, evidence: { path: "docs/receipt.json", sha256: "" },
    artifacts: [{ path: artifactPath, packagePath, itemId, version, bytes: file.bytes,
      sha256: file.sha256, reason: "Synthetic immutable runtime fixture; never a real provenance claim." }],
  };
  const policy = structuredClone(templatePolicy);
  function persist() {
    const receiptBytes = JSON.stringify(receipt);
    write(registry.evidence.path, receiptBytes);
    registry.evidence.sha256 = hash(receiptBytes);
    const registryBytes = JSON.stringify(registry);
    write("docs/registry.json", registryBytes);
    policy.immutableRuntimeArtifacts = { path: "docs/registry.json", sha256: hash(registryBytes) };
  }
  write(artifactPath, content);
  persist();
  return { root, packagePath, artifactPath, content, receipt, registry, policy, write, persist };
}
