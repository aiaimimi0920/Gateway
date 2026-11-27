// Classify bounded CLI events. Persist booleans/hashes, never arbitrary model output or commands.
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { isDeepStrictEqual } from "node:util";

const clean = (value) => String(value ?? "").replaceAll("\r\n", "\n");
export const sha256 = (value) => createHash("sha256").update(value).digest("hex");

export function scenarioEvidence(events, scenario) {
  const commands = events.filter((e) => e.type === "item.completed" && e.item?.type === "command_execution");
  const classified = commands.map(({ item }, index) => {
    const step = scenario.steps[index], output = clean(item.aggregated_output), command = String(item.command ?? "");
    const call = step?.arg?.split(" ");
    const scriptCalled = !call || new RegExp(`\\bnode(?:\\.exe)?(?:["'])?\\s+["']?${call[0].replaceAll(".", "\\.")}` +
      `["']?${call[1] ? `\\s+${call[1]}\\b` : "(?:\\s|[\"']|$)"}`, "i").test(command);
    const success = item.exit_code === 0 && item.status === "completed";
    const matched = !!step && (step.exit === 0 ? success : item.exit_code === step.exit) && scriptCalled &&
      (!step.includes || output.includes(step.includes)) && (!step.also || output.includes(step.also)) &&
      (!step.empty || output.trim() === "") &&
      (!step.readAll || (/\b(Get-Content|ReadAllText|cat|type)\b/i.test(command) &&
        step.readAll.every((text) => output.includes(clean(text))))) &&
      (!step.edit || (command.includes(step.edit) &&
        /\b(WriteAllText|Set-Content|writeFileSync|writeFile|apply_patch)\b/i.test(command)));
    return { id: item.id, exitCode: item.exit_code, status: item.status, expectedExit: step?.exit,
      outputBytes: Buffer.byteLength(output), outputSha256: sha256(output), matched };
  });
  const lastCommand = events.findLastIndex((e) => e.type === "item.completed" && e.item?.type === "command_execution");
  const lastMessage = events.findLastIndex((e) => e.type === "item.completed" && e.item?.type === "agent_message");
  const text = lastMessage >= 0 ? events[lastMessage].item.text : "";
  let exactFinalReply = false;
  try { exactFinalReply = isDeepStrictEqual(JSON.parse(text), scenario.expectedReply); } catch {}
  const orderedTools = commands.length === scenario.steps.length && classified.every((c) => c.matched) &&
    classified.every((c) => typeof c.id === "string" && c.id) &&
    new Set(classified.map((c) => c.id)).size === classified.length;
  return { commands: classified, orderedTools, exactFinalReply,
    finalReplyAfterTools: lastMessage > lastCommand,
    finalReplySha256: typeof text === "string" ? sha256(text) : null,
    turnCompletedOnce: events.filter((e) => e.type === "turn.completed").length === 1,
    errors: events.filter((e) => e.type === "error" || e.type === "turn.failed").map((e) => e.type) };
}

export function fixtureEvidence(workspace, expectedFiles) {
  const real = path.resolve(workspace), parent = fs.lstatSync(real);
  if (parent.isSymbolicLink() || fs.realpathSync(real).toLowerCase() !== real.toLowerCase()) {
    throw new Error("fixture_workspace_changed");
  }
  const names = fs.readdirSync(real).filter((name) => name !== "home").sort();
  const files = {};
  for (const [name, expected] of Object.entries(expectedFiles)) {
    if (path.basename(name) !== name) throw new Error("unsafe_fixture_name");
    const file = path.join(real, name), info = fs.lstatSync(file);
    if (!info.isFile() || info.isSymbolicLink() || info.nlink !== 1 || info.size > 32768 ||
        fs.realpathSync(file).toLowerCase() !== file.toLowerCase()) throw new Error("fixture_file_changed");
    const bytes = fs.readFileSync(file);
    files[name] = { sha256: sha256(bytes), exact: bytes.equals(Buffer.from(expected, "utf8")) };
  }
  return { files, onlyExpectedFiles: isDeepStrictEqual(names, Object.keys(expectedFiles).sort()),
    exactFiles: Object.values(files).every((f) => f.exact) };
}
