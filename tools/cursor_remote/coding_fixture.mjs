// Trusted inputs shared by the remote acceptance and the offline client check.
import fs from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { spawnSync } from "node:child_process";

export const originalSource = "export function add(a, b) {\n  return a - b;\n}\n";
export const expectedSource = "export function add(a, b) {\n  return a + b;\n}\n";
export const expectedTests = 'import test from "node:test";\nimport assert from "node:assert/strict";\nimport { add } from "./calc.mjs";\n' +
  'test("positive", () => assert.equal(add(2, 3), 5));\n' +
  'test("negative", () => assert.equal(add(-4, -7), -11));\n' +
  'test("decimal", () => assert.equal(add(1.25, 2.5), 3.75));\n';

// Never run the contents of a client-writable path with the operator's privileges.
export function verifyTests(source) {
  if (source !== originalSource && source !== expectedSource) throw new Error("untrusted_fixture_source");
  const verification = path.join("C:/Users/Public/nas_home/AI/GameEditor/linshi", `cursor-verify-${randomUUID()}`);
  fs.mkdirSync(verification);
  fs.writeFileSync(path.join(verification, "calc.mjs"), source, { encoding: "utf8", flag: "wx" });
  fs.writeFileSync(path.join(verification, "calc.test.mjs"), expectedTests, { encoding: "utf8", flag: "wx" });
  const env = {};
  for (const name of ["SystemRoot", "WINDIR", "PATH", "PATHEXT"]) {
    const key = Object.keys(process.env).find((k) => k.toLowerCase() === name.toLowerCase());
    if (key) env[key] = process.env[key];
  }
  const run = spawnSync(process.execPath, ["--test", "calc.test.mjs"], {
    cwd: verification, env, windowsHide: true, timeout: 15000, maxBuffer: 128 * 1024, encoding: "utf8",
  });
  const tap = (run.stdout ?? "").replaceAll("\r\n", "\n");
  return { exitCode: run.status, passed: run.status === 0 && /^# pass 3$/m.test(tap) && /^# fail 0$/m.test(tap),
    expectedAssertionFailures: run.status === 1 && /^# fail 3$/m.test(tap) && tap.includes("ERR_ASSERTION"),
    timedOut: !!run.error };
}
