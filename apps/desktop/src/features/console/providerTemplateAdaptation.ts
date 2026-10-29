// Deliberately independent of historical canaries. Only new-flow acceptance may mark a template blue.
export type TemplateAdaptationReceipt = {
  templateId: string;
  evidencePath: string;
  verifiedAt: string;
  flowVersion: 1;
  acceptanceBasis?: "documentation-and-local-tests";
};
export const TEMPLATE_ADAPTATION_RECEIPTS: readonly TemplateAdaptationReceipt[] = [{
  templateId: "nvidia-openai",
  evidencePath: "docs/progress/2026-10-25-nvidia-template-acceptance.md",
  verifiedAt: "2026-10-24T21:43:30Z",
  flowVersion: 1,
}, {
  templateId: "openrouter-openai",
  evidencePath: "docs/progress/2026-10-25-openrouter-template-acceptance.md",
  verifiedAt: "2026-10-25T03:17:27Z",
  flowVersion: 1,
}, {
  templateId: "you",
  evidencePath: "docs/progress/2026-10-25-you-key-reimport-acceptance.md",
  verifiedAt: "2026-10-25T04:54:21Z",
  flowVersion: 1,
}, {
  templateId: "exa",
  evidencePath: "docs/progress/2026-10-25-exa-template-acceptance.md",
  verifiedAt: "2026-10-25T05:59:14Z",
  flowVersion: 1,
}, {
  templateId: "chatgpt",
  evidencePath: "docs/progress/2026-10-25-chatgpt-template-acceptance.md",
  verifiedAt: "2026-10-25T06:50:02Z",
  flowVersion: 1,
}, {
  templateId: "tavily",
  evidencePath: "docs/progress/2026-10-26-tavily-template-acceptance.md",
  verifiedAt: "2026-10-25T17:46:33Z",
  flowVersion: 1,
}, {
  templateId: "linkup",
  evidencePath: "docs/progress/2026-10-26-linkup-template-acceptance.md",
  verifiedAt: "2026-10-25T18:50:43Z",
  flowVersion: 1,
}, {
  templateId: "websearchapi",
  evidencePath: "docs/progress/2026-10-26-websearchapi-template-acceptance.md",
  verifiedAt: "2026-10-25T19:52:54Z",
  flowVersion: 1,
}, {
  templateId: "jina",
  evidencePath: "docs/progress/2026-10-26-jina-template-acceptance.md",
  verifiedAt: "2026-10-25T21:21:22Z",
  flowVersion: 1,
}, {
  templateId: "poe-openai",
  evidencePath: "docs/progress/2026-10-26-poe-template-acceptance.md",
  verifiedAt: "2026-10-26T14:22:15Z",
  flowVersion: 1,
}, {
  templateId: "groq-openai",
  evidencePath: "docs/progress/2026-10-27-groq-template-acceptance.md",
  verifiedAt: "2026-10-26T16:56:53Z",
  flowVersion: 1,
}, {
  templateId: "cohere-chat",
  evidencePath: "docs/progress/2026-10-27-cohere-template-acceptance.md",
  verifiedAt: "2026-10-26T20:43:42Z",
  flowVersion: 1,
}, {
  // 用户批准余额受限验收；不代表生成成功。
  templateId: "openai",
  evidencePath: "docs/progress/2026-10-27-openai-template-acceptance.md",
  verifiedAt: "2026-10-27T04:52:39Z",
  flowVersion: 1,
}, {
  // 用户批准余额受限验收；不代表生成或 SSE 输出成功。
  templateId: "anthropic",
  evidencePath: "docs/progress/2026-10-27-anthropic-template-acceptance.md",
  verifiedAt: "2026-10-27T07:24:06Z",
  flowVersion: 1,
}, {
  templateId: "gemini-api",
  evidencePath: "docs/progress/2026-10-27-gemini-api-template-acceptance.md",
  verifiedAt: "2026-10-27T09:48:05Z",
  flowVersion: 1,
}, {
  // 用户批准文档与本地合同验收；不代表真实模型生成或 SSE 已通过。
  templateId: "azure-api",
  evidencePath: "docs/progress/2026-10-27-azure-api-template-review.md",
  verifiedAt: "2026-10-27T10:50:56Z",
  flowVersion: 1,
  acceptanceBasis: "documentation-and-local-tests",
}, {
  // 用户无有效账号：仅官方文档与本地合同验收，不代表真实生成已通过。
  templateId: "mistral-api",
  evidencePath: "docs/progress/2026-10-27-mistral-api-template-review.md",
  verifiedAt: "2026-10-27T12:20:59Z",
  flowVersion: 1,
  acceptanceBasis: "documentation-and-local-tests",
}, {
  // 文档与本地合同通过；真实目录请求受团队额度或月度消费上限限制。
  templateId: "xai-api",
  evidencePath: "docs/progress/2026-10-27-xai-api-template-review.md",
  verifiedAt: "2026-10-27T12:45:58Z",
  flowVersion: 1,
  acceptanceBasis: "documentation-and-local-tests",
}, {
  // 用户批准余额受限验收：UI 和普通两种请求模式均到达上游并返回 402。
  templateId: "deepseek-api",
  evidencePath: "docs/progress/2026-10-27-deepseek-api-template-acceptance.md",
  verifiedAt: "2026-10-27T14:24:00Z",
  flowVersion: 1,
}, {
  // 无有效 Key：官方 401 可达、表单与本地 wire 合同通过，不代表真实生成。
  templateId: "perplexity-api",
  evidencePath: "docs/progress/2026-10-27-perplexity-api-template-review.md",
  verifiedAt: "2026-10-27T14:57:45Z",
  flowVersion: 1,
  acceptanceBasis: "documentation-and-local-tests",
}, {
  templateId: "gemini-web",
  evidencePath: "docs/progress/2026-10-29-gemini-web-template-acceptance.md",
  verifiedAt: "2026-10-29T14:51:21.5147534Z",
  flowVersion: 1,
}];
export function templateAdaptationComplete(templateId: string): boolean {
  return TEMPLATE_ADAPTATION_RECEIPTS.some((receipt) => receipt.templateId === templateId
    && receipt.flowVersion === 1 && Boolean(receipt.evidencePath.trim()) && Number.isFinite(Date.parse(receipt.verifiedAt)));
}
