// No upstream or credentials: reproduce Node's test runner inside the installed Codex sandbox.
import fs from "node:fs";
import path from "node:path";
import http from "node:http";
import { randomUUID } from "node:crypto";
import { codingClient } from "./coding_client.mjs";

const workspace = path.join("C:/Users/Public/nas_home/AI/GameEditor/linshi", `cursor-test-sandbox-${randomUUID()}`);
fs.mkdirSync(workspace);
fs.writeFileSync(path.join(workspace, "calc.mjs"), "export function add(a, b) { return a + b; }\n");
fs.writeFileSync(path.join(workspace, "calc.test.mjs"),
  'import test from "node:test"; import assert from "node:assert/strict"; import { add } from "./calc.mjs";\n' +
  'test("positive",()=>assert.equal(add(2,3),5));\n');
let count = 0;
const server = http.createServer((req, res) => {
  let bytes = 0;
  req.on("data", (b) => { if ((bytes += b.length) > 512 * 1024) req.destroy(); });
  req.on("end", () => {
    if (req.url !== "/v1/responses" || count >= 2) { res.writeHead(400); res.end(); return; }
    count++;
    const item = count === 1 ? { type: "function_call", id: "fc_fixture", call_id: "call_fixture",
      name: "exec_command", arguments: JSON.stringify({
        cmd: process.argv.includes("--same-process") ? "node calc.test.mjs" : "node --test calc.test.mjs",
        login: false, yield_time_ms: 10000 }),
      status: "completed" } : { type: "message", id: "msg_fixture", role: "assistant", status: "completed",
        content: [{ type: "output_text", text: "Local sandbox observation complete.", annotations: [] }] };
    const response = { id: `resp_fixture_${count}`, object: "response", model: "cursor-personal", status: "completed", output: [item] };
    const events = [
      { type: "response.created", response: { ...response, status: "in_progress", output: [] } },
      { type: "response.output_item.added", output_index: 0, item: { ...item, status: "in_progress" } },
      { type: "response.output_item.done", output_index: 0, item },
      { type: "response.completed", response },
    ];
    res.writeHead(200, { "content-type": "text/event-stream" });
    res.end(events.map((e) => `event: ${e.type}\ndata: ${JSON.stringify(e)}\n\n`).join(""));
  });
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
try {
  const result = await codingClient({ codex: process.argv[2], base: `http://127.0.0.1:${server.address().port}`,
    workspace, token: "local_fixture", timeoutMs: 45000, prompt: "Run the local test fixture." });
  const commands = result.events.filter((e) => e.type === "item.completed" && e.item?.type === "command_execution");
  const item = commands.length === 1 ? commands[0].item : null;
  const tap = (item?.aggregated_output ?? "").replaceAll("\r\n", "\n");
  const testsPassed = item?.status === "completed" && item.exit_code === 0 &&
    /^# pass 1$/m.test(tap) && /^# fail 0$/m.test(tap);
  const passed = count === 2 && result.exitCode === 0 && !result.bounded && !result.sawReconnect && testsPassed;
  const summary = { workspace, count, exitCode: result.exitCode, bounded: result.bounded,
    commandCount: commands.length, commandExit: item?.exit_code ?? null, testsPassed, passed };
  fs.writeFileSync(path.join(workspace, "result.json"), JSON.stringify(summary, null, 2), "utf8");
  console.log(JSON.stringify(summary));
  if (!passed) process.exitCode = 1;
} finally { server.closeAllConnections(); await new Promise((r) => server.close(r)); }
