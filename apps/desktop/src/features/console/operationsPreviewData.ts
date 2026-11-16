import type { ConsoleRequestAudit } from "../../api/contracts";

/** Explicit opt-in visual samples; never persisted or sent to the gateway. */
export function operationsPreviewData(now = Date.now()): ConsoleRequestAudit[] {
  const providers = ["OpenAI · 演示", "Anthropic · 演示", "Google · 演示"];
  const models = ["gpt-4.1", "claude-sonnet-4", "gemini-2.5-pro"];
  return Array.from({ length: 18 }, (_, index) => {
    const failed = index % 5 === 0;
    const createdAt = new Date(now - index * 240000).toISOString();
    const durationMs = failed ? 21000 : 1800 + index * 730;
    const promptTokens = failed ? 0 : 480 + index * 137;
    const completionTokens = failed ? 0 : 120 + index * 59;
    return {
      id: `demo-request-${String(index + 1).padStart(3, "0")}`,
      projectId: "demo-project", apiKeyId: null, userCredentialId: null,
      accessKeyId: null, sourceAccessKeyId: null, sessionId: `demo-session-${index % 4 + 1}`,
      routePolicyId: null, providerAccountId: `demo-provider-${index % 3}`,
      protocolFamily: "openai", endpointKind: "chat", requestedModel: models[index % 3],
      resolvedModel: failed ? null : models[index % 3], modelAlias: null, stream: true,
      status: failed ? "failed" : "completed", upstreamStatus: failed ? 429 : 200,
      durationMs, promptTokens, completionTokens, totalTokens: promptTokens + completionTokens,
      cacheCreationInputTokens: 0, cacheReadInputTokens: failed ? 0 : index * 25,
      clientHasCacheControl: false, autoCacheApplied: false,
      errorSummary: failed ? "演示错误：上游返回 429，请求超过速率限制。此记录为虚拟数据。" : null,
      routeTrace: { selectedCandidate: { providerLabel: providers[index % 3] }, realCredentialRef: `演示账号 ${index % 3 + 1}`, routeAttemptCount: index % 4 === 0 ? 2 : 1, ...(failed ? { errorCode: "rate_limit_exceeded" } : {}) },
      analysisProfile: null, requestArtifactObjectKey: null, responseArtifactObjectKey: null,
      responseId: `demo-response-${index + 1}`, previousResponseId: null, clientDisconnectedAt: null,
      createdAt, completedAt: new Date(Date.parse(createdAt) + durationMs).toISOString(), updatedAt: createdAt,
    };
  });
}
