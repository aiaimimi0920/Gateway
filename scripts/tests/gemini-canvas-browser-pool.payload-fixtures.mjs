import vm from "node:vm";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";

export function fetchPage({ body = Buffer.from("fixture bytes"), status = 200, contentType = null, contentLength = String(body.byteLength), fetchFailure, bodyFailure } = {}) {
  const calls = [];
  let bodyReads = 0;
  const page = {
    async evaluate(callback, argument) {
      // Execute the actual serialized callback without Node lexical bindings.
      const result = await vm.runInNewContext(`(${callback.toString()})(__argument)`, {
        __argument: argument,
        fetch: async (url, options) => {
          calls.push({ url, options: structuredClone(options) });
          if (fetchFailure !== undefined) throw fetchFailure;
          return {
            status, ok: status >= 200 && status < 300,
            headers: { get: (name) => name === "content-type" ? contentType : name === "content-length" ? contentLength : null },
            async arrayBuffer() {
              bodyReads++;
              if (bodyFailure !== undefined) throw bodyFailure;
              return Uint8Array.from(body).buffer;
            },
          };
        },
        btoa: (binary) => Buffer.from(binary, "latin1").toString("base64"),
      }, { timeout: 1000 });
      return structuredClone(result);
    },
  };
  return { page, calls, bodyReads: () => bodyReads };
}

export function navigationResponse({ body = Buffer.from("fixture"), contentType = "application/octet-stream", status = 200, bodyFailure } = {}) {
  return {
    async body() { if (bodyFailure) throw bodyFailure; return body; },
    headers: () => contentType ? { "content-type": contentType, "content-length": String(body.byteLength), "x-fixture": "retained" }
      : { "content-length": String(body.byteLength), "x-fixture": "retained" },
    status: () => status,
    ok: () => status >= 200 && status < 300,
    url: () => "https://fixture.invalid/final",
  };
}

export function navigationFixture({ download = null, response = navigationResponse(), navigationError, closeError, newPageError } = {}) {
  const calls = [];
  const page = {
    fixtureResponse: response,
    async waitForEvent(name, options) {
      calls.push(["event", name, options]);
      if (!download) throw new Error("fixture download event timeout");
      return download;
    },
    async goto(url, options) {
      calls.push(["goto", url, options]);
      if (navigationError) throw navigationError;
      return response;
    },
    async close() { calls.push(["close"]); if (closeError) throw closeError; },
  };
  const entry = {
    context: { async newPage() { calls.push(["newPage"]); if (newPageError) throw newPageError; return page; } },
    page: { async close() { calls.push(["original-close"]); } },
  };
  return { entry, calls };
}

export function fixtureNavigationCapture(page) {
  const result = Promise.resolve().then(() => page.fixtureResponse?.body());
  void result.catch(() => undefined);
  return { ready: Promise.resolve(), result, async stop() {} };
}

export async function downloadFile(t, body) {
  const directory = await mkdtemp(join(tmpdir(), "gateway-browser-payload-"));
  t.after(async () => {
    const target = resolve(directory);
    if (dirname(target) !== resolve(tmpdir()) || !basename(target).startsWith("gateway-browser-payload-")) {
      throw new Error("fixture cleanup escaped temporary root");
    }
    await rm(target, { recursive: true, force: true });
  });
  const file = join(directory, "download.bin");
  await writeFile(file, body);
  return file;
}
