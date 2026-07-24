import { defineConfig, devices } from "@playwright/test";
import path from "node:path";
import { fileURLToPath } from "node:url";

const workspaceDir = path.dirname(fileURLToPath(import.meta.url));
const skipManagedWebServer = process.env.PLAYWRIGHT_SKIP_WEBSERVER === "1";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  reporter: "list",
  webServer: skipManagedWebServer
    ? undefined
    : {
        command: "npm run dev:web -- --host 127.0.0.1",
        cwd: workspaceDir,
        url: "http://127.0.0.1:1425/ui/",
        reuseExistingServer: !process.env.CI,
        timeout: 120_000,
      },
  use: {
    baseURL: process.env.GATEWAY_UI_BASE_URL ?? "http://127.0.0.1:1425/ui/",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      name: "desktop-chromium",
      use: { ...devices["Desktop Chrome"] },
    },
    {
      name: "mobile-chromium",
      use: { ...devices["Pixel 7"] },
    },
  ],
});
