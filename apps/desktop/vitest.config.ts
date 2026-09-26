import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Bound simultaneous jsdom consoles after splitting the integration suites.
    maxWorkers: 4,
    environment: "jsdom",
    environmentOptions: {
      jsdom: {
        url: "https://gateway.test/ui/",
      },
    },
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    restoreMocks: true,
    exclude: ["e2e/**/*.spec.ts", "node_modules/**", "dist/**"],
  },
});
