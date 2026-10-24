import type { ProbePolicy } from "./siteHealth";

/** Public health origins are adapter-owned, never credential-provider destinations. */
export const officialHealthAdapters = {
  "openai-status": { name: "ChatGPT / OpenAI · Codex API", url: "https://status.openai.com/" },
  "anthropic-status": { name: "Claude 官方状态", url: "https://status.claude.com/" },
  "deepseek-status": { name: "DeepSeek 官方状态", url: "https://status.deepseek.com/" },
  "nvidia-status": { name: "NVIDIA Build 官方状态", url: "https://status.ngc.nvidia.com/" },
  "gemini-status": { name: "Gemini API 官方状态", url: "https://aistudio.google.com/status" },
  "openrouter-status": { name: "OpenRouter 官方状态", url: "https://status.openrouter.ai/" },
  "cohere-status": { name: "Cohere 官方状态", url: "https://status.cohere.com/" },
  "groq-status": { name: "Groq 官方状态", url: "https://groqstatus.com/" },
  "poe-status": { name: "Poe API 官方状态", url: "https://status.poe.com/" },
  "perplexity-status": { name: "Perplexity API 官方状态", url: "https://status.perplexity.com/" },
  "linkup-status": { name: "Linkup 官方状态", url: "https://status.linkup.so/" },
  "exa-status": { name: "Exa Search 官方状态", url: "https://status.exa.ai/" },
  "jina-status": { name: "Jina Reader / Search 官方状态", url: "https://status.jina.ai/" },
  "together-status": { name: "Together AI 官方状态", url: "https://status.together.ai/" },
  "mistral-status": { name: "Mistral API 官方状态", url: "https://status.mistral.ai/" },
  "xai-status": { name: "xAI API 官方状态", url: "https://status.x.ai/" },
  "tavily-status": { name: "Tavily API 官方状态", url: "https://status.tavily.com/" },
  "you-status": { name: "You.com API 官方状态", url: "https://status.you.com/" },
  "azure-status": { name: "Azure OpenAI 官方区域状态", url: "https://azure.status.microsoft/en-us/status" },
  "qwen-status": { name: "百炼 / Qwen · 阿里云公开公告", url: "https://status.aliyun.com/" },
} as const;

export function officialHealthAdapter(adapter: string) {
  return Object.hasOwn(officialHealthAdapters, adapter)
    ? officialHealthAdapters[adapter as keyof typeof officialHealthAdapters] : undefined;
}

export function officialHealthStatus(status: string, t: (zh: string, en: string) => string) {
  const labels: Record<string, [string, string]> = {
    operational: ["未报告故障", "No reported outage"],
    degraded_performance: ["性能下降", "Degraded performance"],
    partial_outage: ["部分中断", "Partial outage"],
    major_outage: ["严重中断", "Major outage"],
    under_maintenance: ["维护中", "Under maintenance"],
    information: ["官方信息提示", "Official information notice"],
    warning: ["官方警告", "Official warning"],
    not_available: ["该区域不适用（非故障）", "Not applicable in this region (not an outage)"],
    no_public_incidents: ["未返回进行中的公开公告（不代表服务正常）", "No ongoing public notices returned (not proof of health)"],
    coverage_unknown: ["服务覆盖未确认", "Service coverage unconfirmed"],
    public_notice: ["公开公告（未确认影响本服务）", "Public notice (impact on this service unconfirmed)"],
  };
  const label = Object.hasOwn(labels, status) ? labels[status] : undefined;
  return label ? t(...label) : t("未知", "Unknown");
}

export function officialHealthScopeNote(adapter: string, t: (zh: string, en: string) => string): string | null {
  if (adapter === "azure-status") return t("这是 Azure OpenAI 的公开区域状态，不代表你的订阅、部署或具体模型可用；不适用不等于故障。", "Public Azure OpenAI regional status does not establish subscription, deployment or model availability; not applicable is not an outage.");
  if (adapter === "qwen-status") return t("读取阿里云中国站公开公告；尚未确认百炼 / Qwen 的产品与地域覆盖。空公告不代表服务正常，最多显示 32 条公告，不用于推断模型可用性。", "Reads Alibaba Cloud China-portal public notices; Model Studio / Qwen product and regional coverage is unconfirmed. Empty notices do not prove health. Shows up to 32 notices without inferring model availability.");
  return null;
}

export function selectHealthAdapter(policy: ProbePolicy, adapter: ProbePolicy["adapter"]): ProbePolicy {
  const preset = officialHealthAdapter(adapter);
  if (!preset) return { ...policy, adapter, site_url: officialHealthAdapter(policy.adapter) ? null : policy.site_url };
  return { ...policy, adapter, site_url: preset.url, auth: { kind: "none", token: null },
    monitor_groups: [], model_map: {}, allow_private_network: false };
}
