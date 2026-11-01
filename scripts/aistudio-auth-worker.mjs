/** Private pipe only: the Rust credential owner persists cookies, never browser storageState. */
import { randomUUID } from "node:crypto";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { captureCookieBundle } from "./import-aistudio-http-cookies.mjs";
import { ORIGIN, RPC, USER_AGENT, validateBundle, cookieHeader, requestHeaders } from "./aistudio-http/auth.mjs";
import { createTransport } from "./aistudio-http/transport.mjs";
import { parseWire } from "./aistudio-http/wire.mjs";

export async function readImportModels(bundle, send = createTransport(45000)) {
  validateBundle(bundle);
  const page = await send("bootstrap", `${ORIGIN}/prompts/new_chat`, { headers: { "User-Agent": USER_AGENT,
    Cookie: cookieHeader(bundle, `${ORIGIN}/prompts/new_chat`), Accept: "text/html" } }, 8 * 1024 * 1024);
  const key = /"WIu0Nc":"([^"\\]+)"/.exec(page.text)?.[1];
  if (!key) throw new Error("aistudio_http_bootstrap_key_missing");
  const catalog = await send("models", `${RPC}ListModels`, { method: "POST", body: "[]",
    headers: requestHeaders(bundle, key, `v1_${Buffer.from(randomUUID()).toString("base64")}`) });
  const rows = parseWire(catalog.text)?.[0];
  if (!Array.isArray(rows) || rows.length > 256) throw new Error("aistudio_import_catalog_invalid");
  const models = [...new Set(rows.map(row => row?.[0]?.replace(/^models\//, "")))];
  if (!models.length || models.some(id => typeof id !== "string" || !/^[A-Za-z0-9._-]{1,128}$/.test(id))) {
    throw new Error("aistudio_import_catalog_invalid");
  }
  return models;
}

export async function managedImport(input, dependencies = {}) {
  if (!input || typeof input.tempRoot !== "string" || !path.isAbsolute(input.tempRoot)) {
    throw new Error("aistudio_import_temp_root_required");
  }
  const bundle = validateBundle(await (dependencies.capture ?? captureCookieBundle)({ tempRoot: input.tempRoot,
    progress: () => {} }));
  const models = await readImportModels(bundle, dependencies.send);
  return { ok: true, bundle, models };
}

async function main() {
  let input = "";
  for await (const chunk of process.stdin) {
    input += chunk;
    if (Buffer.byteLength(input) > 4096) throw new Error("aistudio_import_input_limit");
  }
  const output = await managedImport(JSON.parse(input));
  // This stdout is a private subprocess pipe consumed by Rust, not a CLI report or file export.
  process.stdout.write(JSON.stringify(output));
}
if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  main().catch(() => {
    process.stdout.write(JSON.stringify({ ok: false, code: "aistudio_import_failed" }));
    process.exitCode = 1;
  });
}
