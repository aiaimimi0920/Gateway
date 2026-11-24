// Synthetic metadata subprocess; TCP EOF observes termination of an owned descendant.
import process from "node:process";
import { spawn } from "node:child_process";
let raw = "";
for await (const chunk of process.stdin) raw += chunk;
const { mode, probe } = JSON.parse(raw);
const success = { ok: true, signedUrl: "https://asset.fixture.invalid/image.png", outputId: "output-1",
  actionId: null, resolvedActionType: "create_image_uni_1", realmId: "realm-fixture",
  mediaOperation: "image", artifactField: "image", state: "completed" };
if (mode === "tree") {
  spawn(process.execPath, ["--input-type=module", "-e", `
    import net from 'node:net';
    const socket = net.connect(${probe.port}, '127.0.0.1', () => socket.write('ready\\n'));
    socket.on('error', () => process.exit(1));
    socket.on('end', () => process.exit(0));
    setInterval(() => {}, 1000);
  `], { stdio: "ignore" });
  setInterval(() => {}, 1000);
} else {
  if (mode === "invalid") { process.stdout.write("synthetic-private invalid JSON"); }
  else if (mode === "utf8") { process.stdout.write(Buffer.from([0xff])); }
  else {
    if (mode === "legacy") delete success.state;
    if (mode === "stdout") success.padding = "x".repeat(128 * 1024);
    if (mode === "stderr") process.stderr.write("x".repeat(16 * 1024 + 1));
    if (mode === "error") process.stdout.write(JSON.stringify({ ok: false, error: {
      code: "lumalabs_fixture_error", status: 503, message: "fixture generation failed" } }));
    else if (mode === "payment") process.stdout.write(JSON.stringify({ ok: false, error: {
      code: "lumalabs_action_payment_required", status: 402, stage: "action",
      message: "LumaLabs declined the action: payment or additional credits are required." } }));
    else process.stdout.write(JSON.stringify(success));
    if (mode === "nonzero") process.exitCode = 23;
  }
}
