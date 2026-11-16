// Whole-entry algorithm fixture; real native admission is tested separately with isolated CDP.
import vm from "node:vm";
import { readFileSync, existsSync } from "node:fs";
const entry = new URL("../../udio-browser-worker.mjs", import.meta.url);
const mode = process.env.UDIO_GENERATION_FIXTURE;
const bindings = { process, Buffer, URL, setTimeout, clearTimeout };
for (const name of ["request", "responses", "auth", "storage", "browser", "transport", "diagnostics", "captcha", "native-flow", "generation"]) {
  const url = new URL(`../../udio-browser/${name}.mjs`, import.meta.url);
  if (existsSync(url)) Object.assign(bindings, await import(url));
}
let now = Date.now(), polls = 0;
bindings.Date = class extends Date { static now() { return now; } };
const song = (id, ready = false, status = ready ? "finished" : "pending") => ({ id, status,
  finished: ready, ...(ready && { audio_url: `https://asset.invalid/${id}` }) });
const outcome = (body) => ({ ok: true, status: 200, contentType: "application/json", text: JSON.stringify(body) });
const page = { url: () => "https://www.udio.com/create", waitForTimeout: async (ms) => { now += ms; } };
const context = { pages: () => [page] };
const browser = { contexts: () => [context], close: async () => {
  process.stderr.write("fixture:close\n");
  if (mode === "close-error") throw new Error("private close fixture");
} };
Object.assign(bindings, {
  chromium: { connectOverCDP: async () => browser }, debugLog: async () => {},
  resolveWorkerPage: async () => page,
  resolveUdioAccessToken: async () => { if (mode === "late") now += 60000; return null; },
  resolveVercelCheckpointInBorrowedPage: async () => {},
  submitGenerationThroughPageUi: async () => {
    process.stderr.write("fixture:submit\n");
    if (mode === "challenge") return { ok: false, status: 403, contentType: "text/html", text: "Vercel Security Checkpoint" };
    if (mode === "bad-ids") return outcome({ track_ids: ["A", "A"], songs: [] });
    if (mode === "bad-song") return outcome({ track_ids: ["A", "B"], songs: [song("A"), {}] });
    return outcome({ track_ids: ["A", "B"], songs: [song("A"), song("B")] });
  },
  browserFetch: async () => {
    process.stderr.write("fixture:poll\n"); polls++;
    if (mode === "partial") return outcome({ songs: polls === 1 ? [song("A", true)] : [song("B", true), song("A", true)] });
    if (mode === "failed") return outcome({ songs: [song("A", true), song("B", true, "failed")] });
    if (mode === "foreign") return outcome({ songs: [song("other", true)] });
    if (mode === "disjoint") return outcome({ songs: [song(polls % 2 ? "A" : "B", true)] });
    return outcome({ songs: [song("A"), song("B")] });
  },
});
vm.createContext(bindings);
const source = readFileSync(entry, "utf8").replace(/^import[\s\S]*?from "[^"]+";\r?\n/gm, "");
vm.runInContext(source, bindings);
