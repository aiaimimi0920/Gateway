// Acceptance evidence is classified from client events; never persist commands or output.
export function developmentCommands(events, originalSource, expectedTests) {
  const clean = (text) => String(text ?? "").replaceAll("\r\n", "\n");
  const commands = events.filter((e) => e.type === "item.completed" && e.item?.type === "command_execution");
  const evidence = commands.map(({ item }) => {
    const command = String(item.command ?? ""), output = clean(item.aggregated_output);
    const successful = item.status === "completed" && item.exit_code === 0;
    return {
      id: item.id, exitCode: item.exit_code, status: item.status,
      failureCodes: successful ? [] : [...new Set(output.match(/\b(?:EPERM|EACCES|ENOENT|ERR_ASSERTION|CommandNotFoundException)\b/g) ?? [])],
      read: successful && /\b(Get-Content|ReadAllText|cat|type)\b/i.test(command) &&
        output.includes(clean(originalSource).trim()) && output.includes(clean(expectedTests).trim()),
      edit: successful && command.includes("calc.mjs") &&
        /\b(WriteAllText|Set-Content|writeFileSync|writeFile|apply_patch)\b/i.test(command),
      test: successful && /\bnode(?:\.exe)?(?:["'])?\s+(?:--test\s+)?["']?calc\.test\.mjs\b/i.test(command) &&
        /^# pass 3$/m.test(output) && /^# fail 0$/m.test(output),
    };
  });
  // Separate successful executions, in order; a combined command cannot prove three phases.
  let phase = 0;
  const used = new Set();
  for (const item of evidence) {
    if (typeof item.id === "string" && item.id && !used.has(item.id) && item[["read", "edit", "test"][phase]]) {
      used.add(item.id);
      phase++;
      if (phase === 3) break;
    }
  }
  const lastCommand = events.findLastIndex((e) => e.type === "item.completed" && e.item?.type === "command_execution");
  const finalReplyAfterTools = lastCommand >= 0 && events.slice(lastCommand + 1).some((e) =>
    e.type === "item.completed" && e.item?.type === "agent_message" &&
    typeof e.item.text === "string" && e.item.text.trim().length > 0);
  return { commands: evidence, orderedReadEditTest: phase === 3, finalReplyAfterTools };
}
