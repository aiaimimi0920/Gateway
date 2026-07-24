import { expect, test } from "@playwright/test";

test.describe.configure({ mode: "serial" });

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (!value || value.trim().length === 0) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

test("signs into a live Gateway browser console and reads /v1/models", async ({ page, request }) => {
  const managementToken = requiredEnvironment("GATEWAY_LIVE_MANAGEMENT_TOKEN");
  const apiBaseUrl = requiredEnvironment("GATEWAY_LIVE_API_BASE_URL");
  const apiToken = requiredEnvironment("GATEWAY_LIVE_API_TOKEN");
  const expectedProviderId = requiredEnvironment("GATEWAY_LIVE_EXPECT_PROVIDER_ID");
  const expectedModel = requiredEnvironment("GATEWAY_LIVE_EXPECT_MODEL");

  await page.goto("/ui/");

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await page.getByLabel("Management token").fill(managementToken);
  await page.getByRole("button", { name: "Sign in" }).click();

  await expect(page.getByRole("heading", { name: "Gateway Web Console" })).toBeVisible();
  await expect(
    page
      .locator("article")
      .filter({ has: page.getByRole("heading", { name: "当前路由配置" }) })
      .getByText(expectedProviderId, { exact: true }),
  ).toBeVisible();
  await expect(page.getByLabel("Route document JSON")).toContainText(expectedModel);

  const modelsResponse = await request.get(`${apiBaseUrl}/v1/models`, {
    headers: { Authorization: `Bearer ${apiToken}` },
  });
  expect(modelsResponse.ok()).toBeTruthy();
  const modelsPayload = (await modelsResponse.json()) as {
    data?: Array<{ id?: string }>;
  };
  const modelIds = (modelsPayload.data ?? [])
    .map((entry) => entry.id)
    .filter((entry): entry is string => typeof entry === "string");
  expect(modelIds).toContain(expectedModel);
});

test("saves a live provider update through the browser console and extends /v1/models", async ({ page, request }) => {
  const managementToken = requiredEnvironment("GATEWAY_LIVE_MANAGEMENT_TOKEN");
  const apiBaseUrl = requiredEnvironment("GATEWAY_LIVE_API_BASE_URL");
  const apiToken = requiredEnvironment("GATEWAY_LIVE_API_TOKEN");
  const addedProviderId = "backup-provider";
  const addedModelId = "gpt-5.4-mini";

  await page.goto("/ui/");

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await page.getByLabel("Management token").fill(managementToken);
  await page.getByRole("button", { name: "Sign in" }).click();

  await expect(page.getByRole("heading", { name: "Gateway Web Console" })).toBeVisible();
  await page.getByRole("button", { name: "Add provider row" }).click();
  await page.getByLabel("Provider id 2").fill(addedProviderId);
  await page.getByLabel("Provider preset 2").fill("openai");
  await page.getByLabel("Provider base URL 2").fill("https://api.backup.example.com");
  await page.getByLabel("Provider supported models 2").fill(addedModelId);

  await expect(page.getByLabel("Route document JSON")).toContainText(`"id": "${addedProviderId}"`);
  await expect(page.getByLabel("Route document JSON")).toContainText(`"${addedModelId}"`);

  await page.getByRole("button", { name: "Save route config" }).click();

  await expect(
    page.getByRole("status", { name: "Gateway console last action" }),
  ).toContainText("Saved route config as active revision");
  await expect(
    page
      .locator("article")
      .filter({ has: page.getByRole("heading", { name: "当前路由配置" }) })
      .getByText(addedProviderId, { exact: true }),
  ).toBeVisible();

  await expect
    .poll(async () => {
      const modelsResponse = await request.get(`${apiBaseUrl}/v1/models`, {
        headers: { Authorization: `Bearer ${apiToken}` },
      });
      if (!modelsResponse.ok()) {
        return [];
      }
      const modelsPayload = (await modelsResponse.json()) as {
        data?: Array<{ id?: string }>;
      };
      return (modelsPayload.data ?? [])
        .map((entry) => entry.id)
        .filter((entry): entry is string => typeof entry === "string");
    })
    .toContain(addedModelId);
});
