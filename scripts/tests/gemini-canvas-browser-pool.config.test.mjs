import assert from "node:assert/strict";
import { createHash, createPrivateKey, X509Certificate } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const { loadOrCreateTlsCertificate, parseBoolean, resolveExecutablePath } = await importTestableScript();
const hash = (value) => createHash("sha256").update(value).digest("hex");

function temporaryRoot(t) {
  const base = fs.realpathSync(os.tmpdir());
  const root = fs.mkdtempSync(path.join(base, "gateway-browser-config-"));
  t.after(() => {
    const resolved = fs.realpathSync(root);
    assert.equal(path.dirname(resolved), base);
    assert.ok(path.basename(resolved).startsWith("gateway-browser-config-"));
    assert.equal(fs.lstatSync(root).isSymbolicLink(), false);
    fs.rmSync(resolved, { recursive: true });
  });
  return root;
}

function tlsRoot(t, root) {
  const previous = process.env.GEMINI_CANVAS_BROWSER_TLS_DIR;
  process.env.GEMINI_CANVAS_BROWSER_TLS_DIR = root;
  t.after(() => {
    if (previous === undefined) delete process.env.GEMINI_CANVAS_BROWSER_TLS_DIR;
    else process.env.GEMINI_CANVAS_BROWSER_TLS_DIR = previous;
  });
}

test("browser boolean configuration preserves accepted values and fallback identity", () => {
  const fallback = { unspecified: true };
  for (const value of ["1", "TRUE", " yes ", "on"]) assert.equal(parseBoolean(value, fallback), true);
  for (const value of ["0", "FALSE", " no ", "off"]) assert.equal(parseBoolean(value, fallback), false);
  for (const value of [undefined, null, true, 1, "", "maybe"]) assert.equal(parseBoolean(value, fallback), fallback);
});

test("browser executable override wins only when its normalized path exists", (t) => {
  const root = temporaryRoot(t), executable = path.join(root, "fixture-browser.exe");
  fs.writeFileSync(executable, "fixture; never executed\n");
  assert.equal(resolveExecutablePath(`  ${executable}  `), executable);
  const defaultPath = resolveExecutablePath(null);
  assert.equal(resolveExecutablePath(path.join(root, "absent.exe")), defaultPath);
  assert.equal(resolveExecutablePath("  "), defaultPath);
  if (defaultPath !== null) assert.equal(fs.existsSync(defaultPath), true);
});

test("TLS configuration generates a matching certificate and reuses persisted bytes", (t) => {
  const root = temporaryRoot(t);
  tlsRoot(t, path.relative(process.cwd(), root));
  const first = loadOrCreateTlsCertificate("localhost");
  assert.equal(first.generated, true);
  assert.equal(first.keyPath, path.join(root, "localhost.key.pem"));
  assert.equal(first.certPath, path.join(root, "localhost.cert.pem"));
  const certificate = new X509Certificate(first.cert);
  assert.equal(certificate.checkPrivateKey(createPrivateKey(first.key)), true);
  assert.equal(certificate.checkHost("localhost"), "localhost");
  assert.equal(certificate.checkIP("127.0.0.1"), "127.0.0.1");
  const second = loadOrCreateTlsCertificate("localhost");
  assert.equal(second.generated, false);
  assert.equal(hash(second.key), hash(first.key));
  assert.equal(hash(second.cert), hash(first.cert));
  assert.equal(hash(fs.readFileSync(first.keyPath)), hash(first.key));
  assert.equal(hash(fs.readFileSync(first.certPath)), hash(first.cert));
});

test("TLS configuration replaces incomplete pairs and retains numeric-host SANs", (t) => {
  const root = temporaryRoot(t);
  tlsRoot(t, root);
  const keyPath = path.join(root, "127.0.0.2.key.pem");
  fs.writeFileSync(keyPath, "incomplete fixture pair\n");
  const bundle = loadOrCreateTlsCertificate("127.0.0.2");
  assert.equal(bundle.generated, true);
  const certificate = new X509Certificate(bundle.cert);
  assert.equal(certificate.checkPrivateKey(createPrivateKey(bundle.key)), true);
  assert.equal(certificate.checkIP("127.0.0.2"), "127.0.0.2");
  assert.equal(certificate.checkIP("127.0.0.1"), "127.0.0.1");
  assert.equal(certificate.checkHost("localhost"), "localhost");
  assert.equal(hash(fs.readFileSync(keyPath)), hash(bundle.key));
});

test("TLS configuration sanitizes reused asset names and propagates directory errors", (t) => {
  const root = temporaryRoot(t);
  tlsRoot(t, root);
  fs.writeFileSync(path.join(root, "a_b_host.key.pem"), "fixture key");
  fs.writeFileSync(path.join(root, "a_b_host.cert.pem"), "fixture cert");
  assert.equal(loadOrCreateTlsCertificate("a/b:host").generated, false);
  const blocked = path.join(root, "file-not-directory");
  fs.writeFileSync(blocked, "fixture\n");
  process.env.GEMINI_CANVAS_BROWSER_TLS_DIR = blocked;
  assert.throws(() => loadOrCreateTlsCertificate("localhost"), (error) => ["EEXIST", "ENOTDIR"].includes(error.code));
});
