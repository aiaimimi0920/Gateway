import { expect, test } from "@playwright/test";

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
