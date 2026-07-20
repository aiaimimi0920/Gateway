import { defineConfig } from "@rsbuild/core";
import { pluginReact } from "@rsbuild/plugin-react";

export default defineConfig({
  plugins: [pluginReact()],
  source: {
    entry: {
      index: "./src/main.tsx",
    },
  },
  html: {
    title: "Neuro Gateway",
    mountId: "root",
  },
  server: {
    port: 1425,
    strictPort: true,
  },
});
