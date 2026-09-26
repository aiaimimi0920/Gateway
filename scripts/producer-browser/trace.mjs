import { mkdir } from "node:fs/promises";
import path from "node:path";

const TRACE_PATH = normalizeTracePath(process.env.PRODUCER_BROWSER_TRACE_PATH);

function normalizeTracePath(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export async function appendTrace(event, details = {}) {
  if (!TRACE_PATH) {
    return;
  }
  const line = JSON.stringify({
    ts: new Date().toISOString(),
    event,
    ...details,
  });
  await mkdir(path.dirname(TRACE_PATH), { recursive: true }).catch(() => undefined);
  await import("node:fs/promises").then(({ appendFile }) =>
    appendFile(TRACE_PATH, `${line}\n`, "utf8").catch(() => undefined),
  );
}
