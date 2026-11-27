// Trusted, disposable fixtures for real Codex acceptance. Never execute model-written files here.
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

export const matrixCases = ["output-edges", "six-rounds", "multi-file", "fresh-text"];
const sumOriginal = "export function sum(values) {\n  return values.reduce((total, value) => total - value, 0);\n}\n";
export const sumFixed = "export function sum(values) {\n  return values.reduce((total, value) => total + value, 0);\n}\n";
const labelOriginal = "export function label(value) {\n  return value;\n}\n";
export const labelFixed = "export function label(value) {\n  return value.trim();\n}\n";
const tests = 'import test from "node:test";\nimport assert from "node:assert/strict";\n' +
  'import { sum } from "./sum.mjs";\nimport { label } from "./label.mjs";\n' +
  'test("sum positive", () => assert.equal(sum([2, 3]), 5));\n' +
  'test("sum negative", () => assert.equal(sum([-4, -7]), -11));\n' +
  'test("sum decimal", () => assert.equal(sum([1.25, 2.5]), 3.75));\n' +
  'test("label spaces", () => assert.equal(label("  ready  "), "ready"));\n' +
  'test("label tabs", () => assert.equal(label("\\tready\\t"), "ready"));\n' +
  'test("label chinese", () => assert.equal(label("  中文  "), "中文"));\n';
const policy = "Use only declared exec_command for all local operations, with login:false and " +
  "yield_time_ms:30000. Stay inside this workspace. No network, other directories, escalation, " +
  "Cursor built-in tools, retries, or sandbox changes. Do not create or modify files unless explicitly allowed. " +
  "Execute each requested command once in a separate tool call, serially. Do not combine commands. " +
  "Use max_output_tokens:8192. A specified nonzero exit is an expected test result, not a reason to stop. ";

export function makeScenario(name) {
  if (!matrixCases.includes(name)) throw new Error("unknown_matrix_case");
  const nonce = randomUUID();
  if (name === "output-edges") {
    const unicode = JSON.stringify({ token: nonce, text: '中文🤖<&amp;>"\\\t\r\n下一行', pad: "abcdef0123456789".repeat(256) });
    const marker = `EXPECTED_FAILURE_${randomUUID()}`, recovery = `RECOVERED_${randomUUID()}`;
    const script = `const mode = process.argv[2];\n` +
      `if (mode === "unicode") console.log(${JSON.stringify(unicode)});\n` +
      `else if (mode === "empty") { /* successful empty output */ }\n` +
      `else if (mode === "failure") { console.error(${JSON.stringify(marker)}); process.exitCode = 7; }\n` +
      `else if (mode === "recovery") console.log(${JSON.stringify(recovery)});\n` +
      'else throw new Error("unknown_fixture_mode");\n';
    return { name, files: { "probe.mjs": script }, expectedFiles: { "probe.mjs": script },
      steps: [
        { arg: "probe.mjs unicode", exit: 0, includes: unicode },
        { arg: "probe.mjs empty", exit: 0, empty: true },
        { arg: "probe.mjs failure", exit: 7, includes: marker },
        { arg: "probe.mjs recovery", exit: 0, includes: recovery },
      ], expectedReply: { token: nonce, failureExit: 7, recovery },
      prompt: policy + "Run in order: node probe.mjs unicode; node probe.mjs empty; " +
        "node probe.mjs failure; exit $LASTEXITCODE (these two statements MUST be in the same tool call " +
        "to preserve native exit 7 through PowerShell); node probe.mjs recovery. " +
        "The first output contains literal Unicode, escaped whitespace and about 4 KiB padding. " +
        "Finally reply only JSON with token from the first output, failureExit, and recovery from the last output." };
  }
  if (name === "six-rounds") {
    const tokens = Array.from({ length: 6 }, () => randomUUID());
    const script = `const tokens = ${JSON.stringify(tokens)};\nconst index = Number(process.argv[2]);\n` +
      'if (!Number.isInteger(index) || index < 1 || index > 6) throw new Error("bad_step");\n' +
      'console.log(JSON.stringify({ step: index, token: tokens[index - 1] }));\n';
    return { name, files: { "steps.mjs": script }, expectedFiles: { "steps.mjs": script },
      steps: tokens.map((token, i) => ({ arg: `steps.mjs ${i + 1}`, exit: 0,
        includes: JSON.stringify({ step: i + 1, token }) })),
      expectedReply: { tokens },
      prompt: policy + "Run node steps.mjs 1, then 2, then 3, then 4, then 5, then 6, each in its own call. " +
        "Do not read steps.mjs directly. Retain the six returned tokens across tool continuations. " +
        "Finally reply only JSON with a tokens array containing all six exact returned tokens in order." };
  }
  if (name === "multi-file") {
    return { name, files: { "sum.mjs": sumOriginal, "label.mjs": labelOriginal, "suite.mjs": tests },
      expectedFiles: { "sum.mjs": sumFixed, "label.mjs": labelFixed, "suite.mjs": tests },
      steps: [
        { readAll: [sumOriginal.trim(), labelOriginal.trim(), tests.trim()], exit: 0 },
        { arg: "suite.mjs", exit: 1, includes: "# fail 6" },
        { edit: "sum.mjs", exit: 0 },
        { edit: "label.mjs", exit: 0 },
        { arg: "suite.mjs", exit: 0, includes: "# pass 6", also: "# fail 0" },
      ], expectedReply: { passed: 6, failed: 0 },
      prompt: policy + "First read sum.mjs, label.mjs and suite.mjs in one call. " +
        "Next run node suite.mjs (all six assertions are expected to fail). Do not use node --test. " +
        "Then edit ONLY sum.mjs in one call, then ONLY label.mjs in another call, using UTF-8 without BOM. " +
        `Required sum.mjs bytes: ${JSON.stringify(sumFixed)}. Required label.mjs bytes: ${JSON.stringify(labelFixed)}. ` +
        "Do not modify suite.mjs. Run node suite.mjs again. Finally reply only JSON with passed and failed counts." };
  }
  return { name, files: {}, expectedFiles: {}, steps: [], expectedReply: { fresh: nonce },
    prompt: policy + `This is a new independent task. Do not call any tool. Reply only this JSON: ${JSON.stringify({ fresh: nonce })}` };
}

// Only these module-owned constants run outside Codex's OS sandbox.
export function verifyMultiFileFixture(fixed) {
  const workspace = path.join("C:/Users/Public/nas_home/AI/GameEditor/linshi", `cursor-matrix-trusted-${randomUUID()}`);
  fs.mkdirSync(workspace);
  for (const [file, text] of Object.entries({
    "sum.mjs": fixed ? sumFixed : sumOriginal, "label.mjs": fixed ? labelFixed : labelOriginal, "suite.mjs": tests,
  })) fs.writeFileSync(path.join(workspace, file), text, { encoding: "utf8", flag: "wx" });
  const env = {};
  for (const name of ["SystemRoot", "WINDIR", "PATH", "PATHEXT"]) {
    const key = Object.keys(process.env).find((k) => k.toLowerCase() === name.toLowerCase());
    if (key) env[key] = process.env[key];
  }
  const run = spawnSync(process.execPath, ["suite.mjs"], {
    cwd: workspace, env, encoding: "utf8", timeout: 10000, maxBuffer: 65536, windowsHide: true,
  });
  const output = (run.stdout ?? "").replaceAll("\r\n", "\n");
  return { exitCode: run.status, timedOut: !!run.error,
    passed: run.status === 0 && /^# pass 6$/m.test(output) && /^# fail 0$/m.test(output),
    expectedFailures: run.status === 1 && /^# pass 0$/m.test(output) && /^# fail 6$/m.test(output) };
}
