import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

test.describe.configure({ mode: "serial" });

const REQUIRED_LIVE_ENVIRONMENT = [
  "GATEWAY_LIVE_MANAGEMENT_TOKEN",
  "GATEWAY_LIVE_API_BASE_URL",
  "GATEWAY_LIVE_API_TOKEN",
  "GATEWAY_LIVE_EXPECT_PROVIDER_ID",
  "GATEWAY_LIVE_EXPECT_MODEL",
  "GATEWAY_LIVE_EXPECT_ADDED_PROVIDER_ID",
  "GATEWAY_LIVE_EXPECT_ADDED_MODEL",
  "GATEWAY_LIVE_EXPECT_RESTORED_MODEL",
  "GATEWAY_LIVE_EXPECT_REMOVED_MODEL",
  "GATEWAY_LIVE_UPSTREAM_BASE_URL",
  "GATEWAY_LIVE_CHAT_MODEL",
  "GATEWAY_LIVE_CHAT_EXPECT_TEXT",
] as const;

test.beforeEach(() => {
  const missingEnvironment = REQUIRED_LIVE_ENVIRONMENT.filter((name) => {
    const value = process.env[name];
    return !value || value.trim().length === 0;
  });

  test.skip(
    missingEnvironment.length > 0,
    `Live Gateway E2E requires environment variables: ${missingEnvironment.join(", ")}`,
  );
});

type ModelsPayload = {
  data?: Array<{ id?: string }>;
};

type ChatCompletionsPayload = {
  model?: string;
  choices?: Array<{
    message?: {
      content?: string;
    };
    finish_reason?: string;
  }>;
};

type RevisionEntry = {
  revision?: {
    id?: string;
    sequence?: number;
    message?: string;
  };
  active?: boolean;
  hasArchive?: boolean;
  source?: string;
};

type RevisionListPayload = {
  revisions?: RevisionEntry[];
};

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (!value || value.trim().length === 0) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

async function signIntoConsole(page: Page, managementToken: string): Promise<void> {
  await page.goto("/ui/");

  await expect(page.getByRole("heading", { name: "登录" })).toBeVisible();
  await page.getByLabel("管理密钥").fill(managementToken);
  await page.getByRole("button", { name: "登录" }).click();

  await expect(page.getByRole("heading", { name: "Gateway 网页控制台" })).toBeVisible();
}

async function fetchModelIds(
  request: APIRequestContext,
  apiBaseUrl: string,
  apiToken: string,
): Promise<string[]> {
  const modelsResponse = await request.get(`${apiBaseUrl}/v1/models`, {
    headers: { Authorization: `Bearer ${apiToken}` },
  });
  expect(modelsResponse.ok()).toBeTruthy();
  const modelsPayload = (await modelsResponse.json()) as ModelsPayload;
  return (modelsPayload.data ?? [])
    .map((entry) => entry.id)
    .filter((entry): entry is string => typeof entry === "string");
}

async function fetchRevisionEntries(
  request: APIRequestContext,
  apiBaseUrl: string,
  managementToken: string,
): Promise<RevisionEntry[]> {
  const revisionsResponse = await request.get(
    `${apiBaseUrl}/v1/internal/gateway/console/revisions`,
    { headers: { "x-management-token": managementToken } },
  );
  expect(revisionsResponse.ok()).toBeTruthy();
  const revisionsPayload = (await revisionsResponse.json()) as RevisionListPayload;
  return revisionsPayload.revisions ?? [];
}

function revisionId(entry: RevisionEntry): string | null {
  const id = entry.revision?.id;
  return typeof id === "string" && id.trim().length > 0 ? id : null;
}

test("signs into a live Gateway browser console and reads /v1/models", async ({ page, request }) => {
  const managementToken = requiredEnvironment("GATEWAY_LIVE_MANAGEMENT_TOKEN");
  const apiBaseUrl = requiredEnvironment("GATEWAY_LIVE_API_BASE_URL");
  const apiToken = requiredEnvironment("GATEWAY_LIVE_API_TOKEN");
  const expectedProviderId = requiredEnvironment("GATEWAY_LIVE_EXPECT_PROVIDER_ID");
  const expectedModel = requiredEnvironment("GATEWAY_LIVE_EXPECT_MODEL");

  await signIntoConsole(page, managementToken);

  await expect(
    page
      .locator("article")
      .filter({ has: page.getByRole("heading", { name: "当前路由配置" }) })
      .getByText(expectedProviderId, { exact: true }),
  ).toBeVisible();
  await expect(page.getByLabel("路由配置 JSON")).toContainText(expectedModel);

  await expect.poll(() => fetchModelIds(request, apiBaseUrl, apiToken)).toContain(expectedModel);
});

test("saves a live provider update through the browser console and extends /v1/models", async ({ page, request }) => {
  const managementToken = requiredEnvironment("GATEWAY_LIVE_MANAGEMENT_TOKEN");
  const apiBaseUrl = requiredEnvironment("GATEWAY_LIVE_API_BASE_URL");
  const apiToken = requiredEnvironment("GATEWAY_LIVE_API_TOKEN");
  const addedProviderId = requiredEnvironment("GATEWAY_LIVE_EXPECT_ADDED_PROVIDER_ID");
  const addedModelId = requiredEnvironment("GATEWAY_LIVE_EXPECT_ADDED_MODEL");

  await signIntoConsole(page, managementToken);

  await page.getByRole("button", { name: "添加 Provider 行" }).click();
  await page.getByLabel("Provider ID 2").fill(addedProviderId);
  await page.getByLabel("Provider 预设 2").fill("openai");
  await page.getByLabel("Provider 基础 URL 2").fill("https://api.backup.example.com");
  await page.getByLabel("Provider 支持模型 2").fill(addedModelId);

  await expect(page.getByLabel("路由配置 JSON")).toContainText(`"id": "${addedProviderId}"`);
  await expect(page.getByLabel("路由配置 JSON")).toContainText(`"${addedModelId}"`);

  await page.getByRole("button", { name: "保存路由配置" }).click();

  await expect(
    page.getByRole("status", { name: "Gateway console last action" }),
  ).toContainText("已将路由配置保存为激活修订");
  await expect(
    page
      .locator("article")
      .filter({ has: page.getByRole("heading", { name: "当前路由配置" }) })
      .getByText(addedProviderId, { exact: true }),
  ).toBeVisible();

  await expect.poll(() => fetchModelIds(request, apiBaseUrl, apiToken)).toContain(addedModelId);
});

test("restores a live archived revision through the browser console", async ({ page, request }) => {
  const managementToken = requiredEnvironment("GATEWAY_LIVE_MANAGEMENT_TOKEN");
  const apiBaseUrl = requiredEnvironment("GATEWAY_LIVE_API_BASE_URL");
  const apiToken = requiredEnvironment("GATEWAY_LIVE_API_TOKEN");
  const expectedProviderId = requiredEnvironment("GATEWAY_LIVE_EXPECT_PROVIDER_ID");
  const restoredModelId = requiredEnvironment("GATEWAY_LIVE_EXPECT_RESTORED_MODEL");
  const addedProviderId = requiredEnvironment("GATEWAY_LIVE_EXPECT_ADDED_PROVIDER_ID");
  const removedModelId = requiredEnvironment("GATEWAY_LIVE_EXPECT_REMOVED_MODEL");

  await expect.poll(() => fetchModelIds(request, apiBaseUrl, apiToken)).toContain(removedModelId);

  await signIntoConsole(page, managementToken);

  const activeConfigCard = page
    .locator("article")
    .filter({ has: page.getByRole("heading", { name: "当前路由配置" }) });
  await expect(activeConfigCard.getByText(addedProviderId, { exact: true })).toBeVisible();

  const archivedRevision = (await fetchRevisionEntries(request, apiBaseUrl, managementToken)).find(
    (entry) => entry.active === false && entry.hasArchive === true && revisionId(entry) !== null,
  );
  const archivedRevisionId = archivedRevision ? revisionId(archivedRevision) : null;
  expect(archivedRevisionId).toBeTruthy();

  await page.getByRole("button", { name: `查看修订 ${archivedRevisionId}` }).click();

  const selectedRevisionDetail = page.getByLabel("Selected revision detail");
  await expect(selectedRevisionDetail.getByText(expectedProviderId, { exact: true })).toBeVisible();
  await expect(page.getByLabel("Selected revision route document snapshot")).toContainText(restoredModelId);
  await expect(page.getByLabel("Selected revision route document snapshot")).not.toContainText(removedModelId);

  await page.getByRole("button", { name: "恢复为激活配置" }).click();

  await expect(page.getByRole("dialog", { name: "确认恢复修订" })).toBeVisible();
  await expect(page.getByRole("dialog", { name: "确认恢复修订" })).toContainText(
    archivedRevisionId ?? "",
  );

  await page.getByRole("button", { name: "确认恢复" }).click();

  await expect(
    page.getByRole("status", { name: "Gateway console last action" }),
  ).toContainText(`已将修订 ${archivedRevisionId} 恢复为激活修订`);
  await expect(activeConfigCard.getByText(addedProviderId, { exact: true })).toHaveCount(0);
  await expect(page.getByLabel("路由配置 JSON")).toContainText(restoredModelId);
  await expect(page.getByLabel("路由配置 JSON")).not.toContainText(removedModelId);

  await expect
    .poll(async () => (await fetchModelIds(request, apiBaseUrl, apiToken)).includes(removedModelId))
    .toBe(false);
  await expect.poll(() => fetchModelIds(request, apiBaseUrl, apiToken)).toContain(restoredModelId);
});

test("routes a live chat completion through a browser-configured provider", async ({ page, request }) => {
  const managementToken = requiredEnvironment("GATEWAY_LIVE_MANAGEMENT_TOKEN");
  const apiBaseUrl = requiredEnvironment("GATEWAY_LIVE_API_BASE_URL");
  const apiToken = requiredEnvironment("GATEWAY_LIVE_API_TOKEN");
  const upstreamBaseUrl = requiredEnvironment("GATEWAY_LIVE_UPSTREAM_BASE_URL");
  const chatModel = requiredEnvironment("GATEWAY_LIVE_CHAT_MODEL");
  const expectedText = requiredEnvironment("GATEWAY_LIVE_CHAT_EXPECT_TEXT");

  await signIntoConsole(page, managementToken);

  await page
    .getByRole("navigation", { name: "Gateway console navigation" })
    .getByRole("button", { name: /路由编辑/ })
    .click();
  await page.getByLabel("Provider 基础 URL 1").fill(upstreamBaseUrl);
  await page.getByLabel("Provider 支持模型 1").fill(chatModel);
  await page.getByLabel("模型路由模式 1").fill(chatModel);

  await expect(page.getByLabel("路由配置 JSON")).toContainText(
    `"base_url": "${upstreamBaseUrl}"`,
  );
  await expect(page.getByLabel("路由配置 JSON")).toContainText(`"${chatModel}"`);

  await page.getByRole("button", { name: "保存路由配置" }).click();

  await expect(
    page.getByRole("status", { name: "Gateway console last action" }),
  ).toContainText("已将路由配置保存为激活修订");
  await expect.poll(() => fetchModelIds(request, apiBaseUrl, apiToken)).toContain(chatModel);

  const chatResponse = await request.post(`${apiBaseUrl}/v1/chat/completions`, {
    headers: { Authorization: `Bearer ${apiToken}` },
    data: {
      model: chatModel,
      messages: [{ role: "user", content: "Say fixture" }],
      stream: false,
    },
  });
  const chatResponseText = await chatResponse.text();
  expect(chatResponse.ok(), chatResponseText).toBeTruthy();

  const chatPayload = JSON.parse(chatResponseText) as ChatCompletionsPayload;
  expect(chatPayload.model).toBe(chatModel);
  expect(chatPayload.choices?.[0]?.message?.content).toContain(expectedText);
  expect(chatPayload.choices?.[0]?.finish_reason).toBe("stop");
});
