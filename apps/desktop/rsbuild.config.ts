import { defineConfig } from "@rsbuild/core";
import { pluginReact } from "@rsbuild/plugin-react";
import { randomUUID } from "node:crypto";
import { rm } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { publishWebDist } from "./tools/publish-web-dist.mjs";

const target = process.env.GATEWAY_UI_TARGET === "web" ? "web" : "tauri";
const isWeb = target === "web";
const stagedPublish = isWeb;
const desktopRoot = path.dirname(fileURLToPath(import.meta.url));
const webStagingRoot = `dist/web-staging-${process.pid}-${randomUUID()}`;
const webStagingPath = path.join(desktopRoot, webStagingRoot);
const webDistRoot = stagedPublish ? webStagingRoot : "dist/web";
const pruneLive =
  isWeb &&
  (process.env.GATEWAY_WEB_PRUNE_LIVE === "1" ||
    !process.argv.includes("--watch"));
// Bind-mounted sources (dev Docker on Windows/macOS) deliver no inotify events,
// so the watcher has to poll. WATCHPACK_POLLING is a webpack-dev-server
// convention that rspack never reads, so set watchOptions.poll explicitly.
const watchPollEnv = Number.parseInt(process.env.GATEWAY_WEB_WATCH_POLL_MS ?? "", 10);
const watchPollInterval =
  Number.isFinite(watchPollEnv) && watchPollEnv > 0 ? watchPollEnv : 1000;

const plugins = [pluginReact()];
if (stagedPublish) {
  plugins.push({
    name: "gateway-web-staged-publish",
    setup(api) {
      api.onAfterBuild(async ({ stats }) => {
        if (!stats || stats.hasErrors()) {
          return;
        }
        await publishWebDist({
          stagingDir: webStagingPath,
          liveDir: path.join(desktopRoot, "dist", "web"),
          readyFile: path.join(desktopRoot, "dist", ".gateway-web-ready"),
          pruneLive,
        });
      });
      api.onCloseBuild(async () => {
        await rm(webStagingPath, {
          recursive: true,
          force: true,
          maxRetries: 5,
          retryDelay: 100,
        });
      });
    },
  });
}

export default defineConfig({
  plugins,
  source: {
    entry: {
      index: "./src/main.tsx",
    },
    define: {
      __GATEWAY_UI_TARGET__: JSON.stringify(target),
    },
  },
  html: {
    title: "Neuro Gateway",
    favicon: "./public/favicon.svg",
    mountId: "root",
  },
  output: {
    assetPrefix: isWeb ? "/ui/" : "./",
    cleanDistPath: true,
    distPath: {
      root: isWeb ? webDistRoot : "dist/tauri",
    },
  },
  server: {
    port: 1425,
    strictPort: true,
  },
  tools: {
    rspack: {
      watchOptions: {
        poll: watchPollInterval,
        ignored: ["**/node_modules/**", "**/dist/**", "**/target/**"],
      },
    },
  },
});
