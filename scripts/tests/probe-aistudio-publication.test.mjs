import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const publicationUrl = new URL("../aistudio-live-probe/publication.mjs", import.meta.url).href;
const childCode = `
  import { writeFile } from "node:fs/promises";
  import path from "node:path";
  import { publishProbeCapture } from ${JSON.stringify(publicationUrl)};
  const [root, mode] = process.argv.slice(1);
  const snapshot = { title: "fixture", url: "https://ai.studio/fixture", bodyText: "fixture",
    textboxes: [], buttons: [], iframes: [], hookEvents: [] };
  const capture = { requests: [{ id: "ordinary", url: snapshot.url }], responses: [], websockets: [] };
  if (mode === "replay") {
    const base = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/";
    const codeBody = Array(21).fill(null);
    codeBody[1] = "fixture-opaque-token";
    codeBody[7] = "models/gemini-fixture";
    codeBody[11] = codeBody[20] = "fixture-app";
    capture.requests = [
      { id: "code", url: base + "CodeAssistantOffline", postDataPreview: JSON.stringify(codeBody) },
      { id: "stream", url: base + "StreamCodeAssistantOfflineGeneration",
        postDataPreview: JSON.stringify(["fixture-generation", null, null, "fixture-app"]) },
    ];
    capture.responses = [
      { requestId: "code", url: capture.requests[0].url, status: 200,
        bodyPreview: JSON.stringify(["fixture-generation"]) },
      { requestId: "stream", url: capture.requests[1].url, status: 200,
        bodyPreview: JSON.stringify([null, "fixture answer"]) },
    ];
  }
  const page = {
    evaluate: async () => snapshot, frames: () => [], url: () => snapshot.url,
    screenshot: async (options) => writeFile(options.path, "synthetic screenshot fixture"),
  };
  await publishProbeCapture({
    page, capture, captureDir: root, initialSnapshot: snapshot,
    input: { runtimeStateObjectKey: "credential-runtime/fixture/storage-state.json", failUnlessTargetRpcCaptured: mode !== "traffic" },
    executablePath: "fixture-browser", runtimeState: { mode: "storage_state_file", absolutePath: "fixture-state" },
    appUrl: snapshot.url, localProxyRequest: null,
    persistCapture: () => writeFile(path.join(root, "capture.json"), JSON.stringify(capture)),
  });
`;

for (const mode of ["traffic", "required", "replay"]) {
  test(`AI Studio publication preserves ${mode} summary, artifact and mirror contracts`, async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "gateway-aistudio-publication-"));
    try {
      const objects = path.join(root, "objects");
      const child = spawnSync(process.execPath, ["--input-type=module", "-e", childCode, root, mode], {
        encoding: "utf8", timeout: 15000,
        env: { ...process.env, AI_GATEWAY_OBJECT_STORAGE_DRIVER: "local", OBJECT_STORAGE_DRIVER: "local",
          AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR: objects },
      });
      assert.equal(child.status, mode === "required" ? 2 : 0, child.stderr);
      const summary = JSON.parse(child.stdout);
      assert.equal(summary.ok, mode !== "required");
      assert.equal(summary.replayReadyTargetRpcContract, mode === "replay");
      assert.deepEqual(JSON.parse(await readFile(path.join(root, "summary.json"), "utf8")), summary);
      const files = await readdir(root);
      for (const name of ["capture.json", "final-page.json", "final-frames.json", "final-page.png",
        "target-rpc-summary.json", "normalized-target-rpc-contract.json", "summary.json"]) {
        assert.ok(files.includes(name), name);
      }
      const capture = JSON.parse(await readFile(path.join(root, "capture.json"), "utf8"));
      assert.equal(capture.finalUrl, "https://ai.studio/fixture");
      assert.equal(capture.targetRpcSummary.replayReadyTargetRpcContract, mode === "replay");
      if (mode === "replay") {
        const mirrorPath = path.join(objects, "credential-runtime/fixture/aistudio-target-rpc-contract.json");
        assert.equal(summary.targetRpcContractMirrorPath, mirrorPath);
        assert.deepEqual(JSON.parse(await readFile(mirrorPath, "utf8")),
          JSON.parse(await readFile(path.join(root, "normalized-target-rpc-contract.json"), "utf8")));
        assert.ok(files.includes("01-code_assistant_offline.json"));
        assert.ok(files.includes("02-stream_code_assistant_offline_generation.json"));
      } else {
        assert.equal(summary.targetRpcContractMirrorPath, null);
        assert.ok(!files.includes("objects"));
      }
    } finally { await rm(root, { recursive: true, force: true }); }
  });
}
