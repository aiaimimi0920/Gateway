// Public target contract only: no Key, automatic region, product fallback or network operation.
export const QWEN_REGIONS = [
  ["cn-beijing", "北京", "Beijing"], ["ap-southeast-1", "新加坡", "Singapore"],
  ["ap-northeast-1", "东京", "Tokyo"], ["eu-central-1", "法兰克福", "Frankfurt"],
  ["us-east-1", "弗吉尼亚", "Virginia"], ["cn-hongkong", "香港", "Hong Kong"],
] as const;
export type QwenEndpointDraft = { region: string; workspaceId: string };
export const emptyQwenEndpoint = (): QwenEndpointDraft => ({ region: "", workspaceId: "" });
export function isQwenProvider(provider: Record<string, unknown> | undefined): boolean {
  return provider?.preset === "qwen-official-api";
}
export function buildQwenEndpoint(value: QwenEndpointDraft): string | null {
  if (!QWEN_REGIONS.some(([region]) => region === value.region) || value.workspaceId === "trial"
    || value.workspaceId.length > 63 || !/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(value.workspaceId)) return null;
  return `https://${value.workspaceId}.${value.region}.maas.aliyuncs.com/compatible-mode/v1`;
}
export function parseQwenEndpoint(base: string): QwenEndpointDraft | null {
  const match = /^https:\/\/([a-z0-9-]+)\.([a-z0-9-]+)\.maas\.aliyuncs\.com\/compatible-mode\/v1\/?$/.exec(base);
  if (!match) return null;
  const value = { workspaceId: match[1], region: match[2] };
  return buildQwenEndpoint(value) ? value : null;
}
export function requireQwenEndpoint(base: string): string {
  const value = parseQwenEndpoint(base);
  if (!value) throw new Error("请选择 Qwen 地域并填写有效工作空间；不能使用 Coding Plan、Web 或任意 API 地址。");
  return buildQwenEndpoint(value)!;
}
// Shared directory origins still require the canonical inference workspace as a separate binding.
export function qwenCatalogEndpoint(base: string): { baseUrl: string; sourceUrl: string } | null {
  const target = parseQwenEndpoint(base);
  if (!target) return null;
  const baseUrl = buildQwenEndpoint(target)!;
  const sourceUrl = target.region === "ap-southeast-1" ? "https://dashscope-intl.aliyuncs.com/api/v1/models"
    : target.region === "cn-hongkong" ? "https://cn-hongkong.dashscope.aliyuncs.com/api/v1/models"
      : baseUrl.replace("/compatible-mode/v1", "/api/v1/models");
  return { baseUrl, sourceUrl };
}
export function qwenCatalogBase(draft: { baseUrl: string; qwenEndpoint?: QwenEndpointDraft }): string {
  const base = requireQwenEndpoint(draft.baseUrl);
  if (draft.qwenEndpoint && buildQwenEndpoint(draft.qwenEndpoint) !== base) {
    throw new Error("Qwen 地域/工作空间与目标地址不一致。");
  }
  return base;
}
