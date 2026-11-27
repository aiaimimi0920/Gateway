import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { makeScenario, matrixCases, verifyMultiFileFixture } from "../../tools/cursor_remote/scenario_cases.mjs";
import { scenarioEvidence } from "../../tools/cursor_remote/scenario_evidence.mjs";

function eventsFor(scenario) {
  return [...scenario.steps.map((s, i) => ({ type: "item.completed", item: {
    id: `tool_${i}`, type: "command_execution", exit_code: s.exit, status: s.exit ? "failed" : "completed",
    command: s.arg ? `node ${s.arg}` : s.edit ? `[IO.File]::WriteAllText('${s.edit}', 'fixed')` : "Get-Content -Raw *.mjs",
    aggregated_output: s.readAll ? s.readAll.join("\n") : [s.includes, s.also].filter(Boolean).join("\n"),
  } })), { type: "item.completed", item: { type: "agent_message", text: JSON.stringify(scenario.expectedReply) } },
  { type: "turn.completed" }];
}
test("matrix has four independent cases and strict ordered tool/final evidence", () => {
  for (const name of matrixCases) {
    const scenario = makeScenario(name), result = scenarioEvidence(eventsFor(scenario), scenario);
    assert.equal(result.orderedTools, true, name);
    assert.equal(result.exactFinalReply, true, name);
    assert.equal(result.finalReplyAfterTools, true, name);
    assert.equal(result.turnCompletedOnce, true, name);
  }
});
test("nonzero exit and empty output must match, not merely a final success statement", () => {
  const scenario = makeScenario("output-edges"), events = eventsFor(scenario);
  for (const change of [
    (e) => { e[2].item.exit_code = 0; },
    (e) => { e[1].item.aggregated_output = "unexpected"; },
    (e) => { e[0].item.aggregated_output = "truncated"; },
    (e) => { e[3].item.command = "echo fabricated"; },
    (e) => { e[2].item.id = e[1].item.id; },
  ]) {
    const changed = structuredClone(events); change(changed);
    assert.equal(scenarioEvidence(changed, scenario).orderedTools, false);
  }
});
test("missing, reordered and combined tools do not pass six continuations", () => {
  const scenario = makeScenario("six-rounds"), events = eventsFor(scenario);
  for (const changed of [
    events.slice(1), [events[1], events[0], ...events.slice(2)], [events[0], ...events.slice(6)],
  ]) assert.equal(scenarioEvidence(changed, scenario).orderedTools, false);
  const early = [events[6], ...events.slice(0, 6), events[7]];
  assert.equal(scenarioEvidence(early, scenario).finalReplyAfterTools, false);
  const wrong = structuredClone(events);
  wrong[6].item.text = JSON.stringify({ tokens: [...scenario.expectedReply.tokens].reverse() });
  assert.equal(scenarioEvidence(wrong, scenario).exactFinalReply, false);
});
test("safe evidence never persists model commands, output, or nonce values", () => {
  const scenario = makeScenario("output-edges"), output = JSON.stringify(scenarioEvidence(eventsFor(scenario), scenario));
  for (const text of ["node probe", scenario.expectedReply.token, scenario.expectedReply.recovery, "abcdef0123"]) {
    assert.equal(output.includes(text), false);
  }
});
test("trusted probe fixtures produce Unicode, empty, explicit failure and recovery without network", () => {
  const scenario = makeScenario("output-edges");
  for (const step of scenario.steps) {
    const run = spawnSync(process.execPath, ["--input-type=module", "-e",
      "process.argv.splice(1, 0, 'probe.mjs');\n" + scenario.files["probe.mjs"], step.arg.split(" ")[1]],
    { encoding: "utf8", windowsHide: true, timeout: 5000, maxBuffer: 32768 });
    assert.equal(run.status, step.exit);
    if (step.empty) assert.equal(run.stdout + run.stderr, "");
    if (step.includes) assert.ok((run.stdout + run.stderr).includes(step.includes));
  }
});
test("trusted multi-file baseline fails six assertions and fixed constants pass all six", () => {
  const before = verifyMultiFileFixture(false), after = verifyMultiFileFixture(true);
  assert.equal(before.expectedFailures, true);
  assert.equal(before.passed, false);
  assert.equal(after.passed, true);
  assert.equal(before.timedOut || after.timedOut, false);
});
test("PowerShell needs explicit native exit propagation and the prompt requests it", () => {
  assert.ok(makeScenario("output-edges").prompt.includes("node probe.mjs failure; exit $LASTEXITCODE"));
  if (process.platform !== "win32") return;
  const command = "node -e 'process.exit(7)'";
  const run = (suffix) => spawnSync("powershell.exe", ["-NoProfile", "-Command", command + suffix],
    { windowsHide: true, timeout: 10000, encoding: "utf8" });
  assert.equal(run("").status, 1);
  assert.equal(run("; exit $LASTEXITCODE").status, 7);
});
