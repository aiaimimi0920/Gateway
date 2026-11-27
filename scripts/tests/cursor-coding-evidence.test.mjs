import test from "node:test";
import assert from "node:assert/strict";
import { developmentCommands } from "../../tools/cursor_remote/coding_evidence.mjs";
import { codingArguments } from "../../tools/cursor_remote/coding_client.mjs";

const source = "original source\n", tests = "trusted tests\n";
const event = (id, command, output, exit = 0) => ({
  type: "item.completed", item: { id, type: "command_execution",
    command, aggregated_output: output, status: "completed", exit_code: exit },
});
const read = event("read", "Get-Content calc.mjs,calc.test.mjs -Raw", source + tests);
const edit = event("edit", "[IO.File]::WriteAllText('calc.mjs', 'fixed')", "");
const run = event("test", 'powershell.exe -Command "node --test calc.test.mjs"', "# pass 3\r\n# fail 0\r\n");
const check = (events) => developmentCommands(events, source, tests);

test("coding evidence requires separate successful ordered read, edit and test events", () => {
  assert.equal(check([read, edit, run]).orderedReadEditTest, true);
  for (const events of [
    [read, run], [edit, read, run], [read, edit], [run, read, edit],
    [read, { ...edit, item: { ...edit.item, id: "read" } }, run],
    [read, event("edit", edit.item.command, "", 1), run],
    [read, edit, event("test", run.item.command, "# pass 3\n# fail 1\n")],
    [event("read", read.item.command, "not the fixture"), edit, run],
  ]) assert.equal(check(events).orderedReadEditTest, false);
  const combined = event("all", `${read.item.command};${edit.item.command};node --test calc.test.mjs`,
    source + tests + "\n# pass 3\n# fail 0\n");
  assert.equal(check([combined]).orderedReadEditTest, false);
});

test("coding evidence contains classifications only, not source or free-form command output", () => {
  const result = JSON.stringify(check([read, edit, run]));
  for (const secret of [source.trim(), tests.trim(), "Get-Content", "WriteAllText", "# pass"]) {
    assert.equal(result.includes(secret), false);
  }
});

test("same-process node:test execution is accepted without relaxing test results or sandbox errors", () => {
  const sameProcess = event("test", "node calc.test.mjs", "# pass 3\n# fail 0\n");
  assert.equal(check([read, edit, sameProcess]).orderedReadEditTest, true);
  const denied = event("test", "node --test calc.test.mjs", "error: 'spawn EPERM'\n", 1);
  assert.equal(check([read, edit, denied]).orderedReadEditTest, false);
  assert.deepEqual(check([denied]).commands[0].failureCodes, ["EPERM"]);
});

test("coding client keeps tools enabled but isolates credentials and disables retries", () => {
  const args = codingArguments("http://127.0.0.1:4315", "C:/fixture", "fix");
  for (const value of [
    "--ignore-user-config", "workspace-write", 'windows.sandbox="unelevated"',
    "model_providers.cursor_acceptance.request_max_retries=0",
    "model_providers.cursor_acceptance.stream_max_retries=0",
    'shell_environment_policy.exclude=["CURSOR_ACCEPTANCE_LOCAL_KEY","*TOKEN*","*SECRET*","*API_KEY*"]',
  ]) assert.ok(args.includes(value), value);
  assert.equal(args.some((v) => v === "features.shell_tool=false" || v === "features.unified_exec=false"), false);
});

test("coding final reply must follow the last tool, not just precede it", () => {
  const message = (text) => ({ type: "item.completed", item: { type: "agent_message", text } });
  assert.equal(check([read, edit, run, message("Three tests passed.")]).finalReplyAfterTools, true);
  for (const events of [
    [message("Starting."), read, edit, run],
    [read, edit, run, message(" \n")],
    [message("No tools.")],
    [read, message("Earlier reply."), edit, run],
  ]) assert.equal(check(events).finalReplyAfterTools, false);
});
