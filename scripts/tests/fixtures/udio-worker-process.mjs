// Synthetic metadata/process fixture only; no browser, credentials or platform calls.
import process from "node:process";
import { spawn } from "node:child_process";
let raw = "";
for await (const chunk of process.stdin) raw += chunk;
const { mode, probe } = JSON.parse(raw);
const success = { ok: true, result: { trackIds: ["fixture"], songs: [], completed: false } };
if (mode === "tree") {
  spawn(process.execPath, ["--input-type=module", "-e", `
    import net from 'node:net';
    const socket = net.connect(${probe.port}, '127.0.0.1', () => socket.write('ready\\n'));
    socket.on('data', () => { socket.end(); process.exit(0); });
    socket.on('error', () => process.exit(1));
    socket.on('end', () => process.exit(0));
    setInterval(() => {}, 1000);
  `], { stdio: "ignore" });
  setInterval(() => {}, 1000);
} else {
  if (mode === "invalid") { process.stdout.write("private invalid JSON"); process.exit(0); }
  if (mode === "empty") { process.stderr.write("private empty diagnostic"); process.exit(0); }
  if (mode === "utf8") { process.stdout.write(Buffer.from([0xff])); process.exit(0); }
  if (mode === "stdout") success.padding = "x".repeat(128 * 1024);
  if (mode === "stderr") process.stderr.write("x".repeat(16 * 1024 + 1));
  if (mode === "error") {
    process.stdout.write(JSON.stringify({ ok: false, error: { status: 503, code: "udio_fixture_error", message: "Bearer synthetic-private-token" } }));
  } else process.stdout.write(JSON.stringify(success));
  if (mode === "nonzero") process.exitCode = 23;
}
