import { defineConfig } from "@rsbuild/core";
import { pluginReact } from "@rsbuild/plugin-react";

const target = process.env.GATEWAY_UI_TARGET === "web" ? "web" : "tauri";
const isWeb = target === "web";

export default defineConfig({
  plugins: [pluginReact()],
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
    mountId: "root",
  },
  output: {
    assetPrefix: isWeb ? "/ui/" : "./",
    cleanDistPath: true,
    distPath: {
      root: isWeb ? "dist/web" : "dist/tauri",
    },
  },
  server: {
    port: 1425,
    strictPort: true,
  },
});
