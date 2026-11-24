// 隔离构建真实 Web 入口，或预览实际 Luma 组件；不发布或清空现有 dist。
import { mkdir } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const { values } = parseArgs({
  options: {
    output: { type: "string" },
    preview: { type: "boolean", default: false },
    "preview-build": { type: "boolean", default: false },
    port: { type: "string", default: "4451" },
    "lifetime-seconds": { type: "string", default: "1800" },
    help: { type: "boolean", default: false },
  },
});
if (values.help) {
  console.log("node tools/luma/build-ui.mjs --output <new-directory> [--preview | --preview-build] [--port 4451] [--lifetime-seconds 1800]");
  process.exit(0);
}
if (!values.output || (values.preview && values["preview-build"])) {
  throw new Error("Specify --output and at most one preview mode.");
}
const port = Number(values.port);
const lifetime = Number(values["lifetime-seconds"]);
if (!Number.isInteger(port) || port < 1 || port > 65535 ||
    !Number.isInteger(lifetime) || lifetime < 1 || lifetime > 86400) {
  throw new Error("Invalid port or lifetime (1..86400 seconds).");
}
const repo = fileURLToPath(new URL("../../", import.meta.url));
const app = path.join(repo, "apps/desktop");
const output = path.resolve(values.output);
await mkdir(path.dirname(output), { recursive: true });
await mkdir(output); // 已有输出目录报错，避免覆盖其他人的结果。
const require = createRequire(path.join(app, "package.json"));
const { createRsbuild, loadConfig } = await import(pathToFileURL(require.resolve("@rsbuild/core")));
process.env.GATEWAY_UI_TARGET = "web";
const { content } = await loadConfig({ cwd: app });
content.plugins = content.plugins.filter(plugin => plugin.name !== "gateway-web-staged-publish");
content.output = { ...content.output, cleanDistPath: false, distPath: { root: output } };
if (values.preview || values["preview-build"]) {
  content.source = {
    ...content.source,
    entry: { index: path.join(app, "e2e/fixtures/luma-media-preview.tsx") },
  };
  content.output.assetPrefix = "/";
  content.server = { host: "127.0.0.1", port, strictPort: true, open: false };
}
const rsbuild = await createRsbuild({ cwd: app, rsbuildConfig: content });
if (!values.preview) {
  await rsbuild.build();
} else {
  const { server } = await rsbuild.startDevServer();
  let closing;
  let timer;
  const close = () => {
    if (!closing) {
      clearTimeout(timer);
      closing = server.close().catch(error => {
        console.error(error);
        process.exitCode = 1;
      });
    }
    return closing;
  };
  process.once("SIGTERM", close);
  process.once("SIGINT", close);
  timer = setTimeout(close, lifetime * 1000);
}
