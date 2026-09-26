import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

const scriptPath = path.resolve(
  import.meta.dirname,
  "..",
  "probe-aistudio-live-request.mjs",
);

test("resolves explicit AI Studio browser proxy direct mode without inheriting system proxy", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { resolveBrowserProxySettings } = module;

    assert.equal(typeof resolveBrowserProxySettings, "function");

    const direct = resolveBrowserProxySettings({ browserProxyUrl: "direct" });
    assert.equal(direct.mode, "direct");
    assert.equal(direct.launchProxy, null);
    assert.deepEqual(direct.launchArgs, ["--no-proxy-server"]);

    const disabled = resolveBrowserProxySettings({ browserProxyUrl: "none" });
    assert.equal(disabled, null);
  `;

  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", childCode],
    {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AISTUDIO_PROBE_SUPPRESS_MAIN: "1",
      },
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stdout || result.stderr);
});

test("resolves explicit AI Studio browser proxy URL and bypass list", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { resolveBrowserProxySettings } = module;

    const proxy = resolveBrowserProxySettings({
      browserProxyUrl: "http://user:pass@127.0.0.1:7890",
      browserProxyBypass: "localhost,127.0.0.1",
    });

    assert.equal(proxy.mode, "proxy");
    assert.deepEqual(proxy.launchArgs, []);
    assert.deepEqual(proxy.launchProxy, {
      server: "http://127.0.0.1:7890",
      username: "user",
      password: "pass",
      bypass: "localhost,127.0.0.1",
    });
  `;

  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", childCode],
    {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AISTUDIO_PROBE_SUPPRESS_MAIN: "1",
      },
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stdout || result.stderr);
});

test("builds Chromium proxy launch options without proxy null in direct mode", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildBrowserProxyLaunchOptions,
      resolveBrowserProxySettings,
    } = module;

    assert.equal(typeof buildBrowserProxyLaunchOptions, "function");

    const options = buildBrowserProxyLaunchOptions(
      resolveBrowserProxySettings({ browserProxyUrl: "direct" }),
    );
    assert.deepEqual(options, { args: ["--no-proxy-server"] });
    assert.equal(Object.hasOwn(options, "proxy"), false);
  `;

  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", childCode],
    {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AISTUDIO_PROBE_SUPPRESS_MAIN: "1",
      },
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stdout || result.stderr);
});

test("detects local browser proxy endpoint for preflight diagnostics", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import net from "node:net";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      assertBrowserProxyReachable,
      resolveBrowserProxySettings,
    } = module;

    assert.equal(typeof assertBrowserProxyReachable, "function");

    const server = net.createServer((socket) => socket.destroy());
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    const port = server.address().port;
    try {
      const result = await assertBrowserProxyReachable(
        resolveBrowserProxySettings({ browserProxyUrl: "http://127.0.0.1:" + port }),
        1000,
      );
      assert.equal(result.ok, true);
      assert.equal(result.host, "127.0.0.1");
      assert.equal(result.port, port);
    } finally {
      await new Promise((resolve) => server.close(resolve));
    }

    await assert.rejects(
      () =>
        assertBrowserProxyReachable(
          resolveBrowserProxySettings({ browserProxyUrl: "http://127.0.0.1:1" }),
          200,
        ),
      /Browser proxy endpoint is not reachable/,
    );
  `;

  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", childCode],
    {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AISTUDIO_PROBE_SUPPRESS_MAIN: "1",
      },
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stdout || result.stderr);
});

test("writes browser proxy diagnostics when launch-time probe fails", () => {
  const captureDir = mkdtempSync(path.join(tmpdir(), "aistudio-probe-proxy-diag-"));
  const objectStoreDir = mkdtempSync(path.join(tmpdir(), "aistudio-probe-proxy-objects-"));
  const runtimeStateRelativePath = path.join(
    "credential-runtime",
    "aistudio",
    "proxy-diag",
    "storage-state.json",
  );
  const runtimeStateObjectKey = runtimeStateRelativePath.replaceAll(path.sep, "/");
  const runtimeStatePath = path.join(objectStoreDir, runtimeStateRelativePath);
  const fakeBrowserPath = path.join(captureDir, "missing-browser.exe");

  try {
    mkdirSync(path.dirname(runtimeStatePath), { recursive: true });
    writeFileSync(
      runtimeStatePath,
      JSON.stringify({ cookies: [], origins: [] }),
      "utf8",
    );
    writeFileSync(fakeBrowserPath, "not a real browser", "utf8");

    const result = spawnSync(process.execPath, [scriptPath], {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR: objectStoreDir,
      },
      input: JSON.stringify({
        runtimeStateObjectKey,
        captureDir,
        browserExecutablePath: fakeBrowserPath,
        browserProxyUrl: "http://127.0.0.1:42344",
      }),
      encoding: "utf8",
    });

    assert.equal(result.status, 1, result.stdout || result.stderr);
    const stdout = JSON.parse(result.stdout);
    assert.equal(stdout.browserProxyMode, "proxy");
    assert.equal(stdout.browserProxyServer, "http://127.0.0.1:42344");
  } finally {
    rmSync(captureDir, { recursive: true, force: true });
    rmSync(objectStoreDir, { recursive: true, force: true });
  }
});

test("local WebSocket capture server close destroys open sockets", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import net from "node:net";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { createLocalWebSocketCaptureServer } = module;

    assert.equal(typeof createLocalWebSocketCaptureServer, "function");

    async function reservePort() {
      const server = net.createServer();
      await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
      const port = server.address().port;
      await new Promise((resolve) => server.close(resolve));
      return port;
    }

    const capture = {};
    const persistCapture = async () => {};
    const port = await reservePort();
    const server = await createLocalWebSocketCaptureServer(port, capture, persistCapture);
    const client = net.createConnection({ host: "127.0.0.1", port });
    await new Promise((resolve) => client.once("connect", resolve));

    // A draining server close may also deliver the client's close event.
    const clientClosedEvent = new Promise((resolve) => client.once("close", () => resolve("closed")));
    const closed = await Promise.race([
      server.close().then(() => "closed"),
      new Promise((resolve) => setTimeout(() => resolve("timeout"), 1000)),
    ]);
    const clientClosed = await Promise.race([
      clientClosedEvent,
      new Promise((resolve) => setTimeout(() => resolve("timeout"), 1000)),
    ]);

    assert.equal(closed, "closed");
    assert.equal(clientClosed, "closed");
  `;

  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", childCode],
    {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AISTUDIO_PROBE_SUPPRESS_MAIN: "1",
      },
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stdout || result.stderr);
});
