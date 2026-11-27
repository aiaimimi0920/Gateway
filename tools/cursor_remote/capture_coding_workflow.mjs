// Run the installed Codex sandbox against a deterministic loopback Responses service.
// The fixture has no upstream URL option and never opens the 4200 data root.
import fs from "node:fs";
import path from "node:path";
import http from "node:http";
import { randomUUID } from "node:crypto";
import { spawnSync } from "node:child_process";
import { codingClient } from "./coding_client.mjs";
import { developmentCommands } from "./coding_evidence.mjs";
import { originalSource, expectedSource, expectedTests, verifyTests } from "./coding_fixture.mjs";

const codex = process.argv[2];
if (!codex || !path.isAbsolute(codex)) throw new Error("absolute_codex_required");
const version = spawnSync(codex, ["--version"], { encoding: "utf8", timeout: 5000, windowsHide: true });
if (version.status !== 0 || version.stdout.trim() !== "codex-cli 0.155.1") throw new Error("wrong_codex_version");
const dir = path.join("C:/Users/Public/nas_home/AI/GameEditor/linshi", `cursor-client-workflow-${randomUUID()}`);
const workspace = path.join(dir, "project");
fs.mkdirSync(workspace, { recursive: true });
const source = path.join(workspace, "calc.mjs"), tests = path.join(workspace, "calc.test.mjs");
fs.writeFileSync(source, originalSource, { flag: "wx" });
fs.writeFileSync(tests, expectedTests, { flag: "wx" });
const token = randomUUID(), finalText = "Local coding fixture complete.";
const calls = [
  ["read", "Get-Content -LiteralPath 'calc.mjs','calc.test.mjs' -Raw"],
  ["edit", "$p = (Resolve-Path -LiteralPath 'calc.mjs').Path; $s = [IO.File]::ReadAllText($p); " +
    "if (-not $s.Contains('a - b')) { throw 'fixture_source_mismatch' }; " +
    "[IO.File]::WriteAllText($p, $s.Replace('a - b','a + b'), [Text.UTF8Encoding]::new($false)); Write-Output FIXTURE_EDIT_OK"],
  ["test", "node calc.test.mjs"],
];
const outputMatches = (stage, output) => {
  const text = typeof output === "string" ? output.replaceAll("\r\n", "\n") : "";
  if (!/^Process exited with code 0$/m.test(text)) return false;
  if (stage === "read") return text.includes(originalSource.trim()) && text.includes(expectedTests.trim());
  if (stage === "edit") return /^FIXTURE_EDIT_OK$/m.test(text);
  return /^# pass 3$/m.test(text) && /^# fail 0$/m.test(text);
};
const receipt = { mode: "loopback-scripted-responses", version: version.stdout.trim(), workspace,
  observedAt: new Date().toISOString(), requests: 0, acknowledged: [], rejected: [], passed: false };
function checkRequest(body, step) {
  if (body.model !== "cursor-personal" || body.stream !== true || !Array.isArray(body.input) ||
      !body.tools?.some((t) => t.type === "function" && t.name === "exec_command")) return false;
  const history = body.input.filter((i) => i.type === "function_call" || i.type === "function_call_output");
  return history.length === 2 * step && calls.slice(0, step).every(([stage], n) => {
    const [call, returned] = history.slice(n * 2, n * 2 + 2);
    return call.type === "function_call" && call.name === "exec_command" && call.call_id === `call_${stage}` &&
      returned.type === "function_call_output" && returned.call_id === call.call_id && outputMatches(stage, returned.output);
  });
}
function writeEvents(res, step) {
  const item = step < calls.length ? { type: "function_call", name: "exec_command", id: `fc_${calls[step][0]}`,
    call_id: `call_${calls[step][0]}`, status: "completed",
    arguments: JSON.stringify({ cmd: calls[step][1], login: false, yield_time_ms: 10000 }) } :
    { type: "message", id: "msg_final", role: "assistant", status: "completed",
      content: [{ type: "output_text", text: finalText, annotations: [] }] };
  const response = { id: `resp_workflow_${step}`, object: "response", model: "cursor-personal", status: "completed", output: [item] };
  const events = [
    { type: "response.created", response: { ...response, status: "in_progress", output: [] } },
    { type: "response.output_item.added", output_index: 0, item: { ...item, status: "in_progress" } },
    { type: "response.output_item.done", output_index: 0, item },
    { type: "response.completed", response },
  ];
  res.writeHead(200, { "content-type": "text/event-stream" });
  res.end(events.map((e) => `event: ${e.type}\ndata: ${JSON.stringify(e)}\n\n`).join(""));
}
const server = http.createServer({ requestTimeout: 12000 }, async (req, res) => {
  if (req.headers.authorization !== `Bearer ${token}`) { res.writeHead(401).end(); return; }
  const step = receipt.requests++;
  try {
    if (req.method !== "POST" || req.url !== "/v1/responses" || step > 3) throw new Error("unexpected_request");
    const chunks = []; let bytes = 0;
    for await (const part of req) {
      if ((bytes += part.length) > 512 * 1024) throw new Error("body_limit");
      chunks.push(part);
    }
    if (!checkRequest(JSON.parse(Buffer.concat(chunks)), step)) throw new Error("invalid_tool_history");
    if (step) receipt.acknowledged.push(calls[step - 1][0]);
    writeEvents(res, step);
  } catch (e) {
    receipt.rejected.push(["unexpected_request", "body_limit", "invalid_tool_history"].includes(e.message) ? e.message : "bad_request");
    res.writeHead(400).end();
  }
});
try {
  receipt.baseline = verifyTests(originalSource);
  if (!receipt.baseline.expectedAssertionFailures) throw new Error("baseline_invalid");
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const cli = await codingClient({ codex, base: `http://127.0.0.1:${server.address().port}`, workspace, token,
    timeoutMs: 90000, prompt: "Read calc.mjs and calc.test.mjs, fix add in calc.mjs only, then run node calc.test.mjs." });
  const evidence = developmentCommands(cli.events, originalSource, expectedTests);
  const finished = cli.events.filter((e) => e.type === "item.completed" && e.item?.type === "agent_message");
  receipt.cli = { exit: cli.exitCode, bounded: cli.bounded, reconnect: cli.sawReconnect,
    finalTextMatched: finished.length === 1 && finished[0].item.text === finalText,
    turnsCompleted: cli.events.filter((e) => e.type === "turn.completed").length,
    errors: cli.events.filter((e) => ["turn.failed", "error"].includes(e.type)).length, ...evidence };
  receipt.sourceMatched = fs.readFileSync(source).equals(Buffer.from(expectedSource));
  receipt.testsUnchanged = fs.readFileSync(tests).equals(Buffer.from(expectedTests));
  receipt.projectFiles = fs.readdirSync(workspace).filter((f) => f !== "home").sort();
  receipt.independentTests = receipt.sourceMatched && receipt.testsUnchanged ? verifyTests(expectedSource) : { passed: false };
  receipt.passed = receipt.requests === 4 && !receipt.rejected.length && receipt.acknowledged.join(",") === "read,edit,test" &&
    cli.exitCode === 0 && !cli.bounded && !cli.sawReconnect && receipt.cli.turnsCompleted === 1 && !receipt.cli.errors &&
    receipt.cli.finalTextMatched && evidence.commands.length === 3 && evidence.orderedReadEditTest &&
    receipt.projectFiles.join(",") === "calc.mjs,calc.test.mjs" && receipt.independentTests.passed;
} finally {
  server.closeAllConnections();
  if (server.listening) await new Promise((resolve) => server.close(resolve));
  const evidencePath = path.join(dir, "result.json");
  fs.writeFileSync(evidencePath, JSON.stringify(receipt, null, 2) + "\n", { flag: "wx" });
  console.log(JSON.stringify({ evidencePath, ...receipt }));
  if (!receipt.passed) process.exitCode = 1;
}
