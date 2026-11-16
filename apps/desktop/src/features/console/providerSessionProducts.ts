/** Stable products own credential meaning and destination; mutable provider IDs do not. */
export const SESSION_PRODUCTS = {
  "grok-web": { preset: "grok", baseUrl: "https://grok.com", cookie: "sso", secondaryCookie: "sso-rw", vendorKey: "xai", vendorName: "Grok", special: "grok_web",
    hint: "token 为 Grok SSO 值，也可导入含 sso 的完整 Cookie；不接收 xAI API Key 或 Grok Build OAuth。" },
  "qwen-web": { preset: "qwen-web-chat", baseUrl: "https://chat.qwen.ai/api/v1", cookie: null, vendorKey: "qwen", vendorName: "Qwen", special: "qwen_web",
    hint: "token 为 Qwen 网页 auth token；可附完整 Cookie。本入口不导入 OAuth、API Key 或账号密码恢复 seed。" },
  "chataibot-web": { preset: "chataibot", baseUrl: "https://chataibot.pro", cookie: "token", vendorKey: "chataibot", vendorName: "ChatAIBot", special: "chataibot_web",
    hint: "token 为 token Cookie 的值，或提供含该项的完整 Cookie。用于图像执行器，不是通用聊天 API。" },
  "lumalabs-web": { preset: "lumalabs", baseUrl: "https://app.lumalabs.ai", cookie: "wos-session", vendorKey: "lumalabs", vendorName: "LumaLabs", special: "lumalabs_web",
    hint: "需 wos-session 值及 realmId（board ID）；这是 uni-1 board Web 渠道，不是 Luma Agents API。" },
  "suno-web": { preset: "suno", baseUrl: "https://studio-api-prod.suno.com", cookie: "__session", vendorKey: "suno", vendorName: "Suno", special: "suno_web",
    hint: "必须提供含 __session 的完整 Cookie；只有短期 Clerk bearer 不足以保存长期会话。" },
  "udio-web": { preset: "udio", baseUrl: "https://www.udio.com", cookie: "sb-ssr-production-auth-token", vendorKey: "udio", vendorName: "Udio", special: "udio_web",
    hint: "token 为 Supabase SSR Cookie 值；分片 Cookie 请保留完整 cookieHeader，不合并或改写分片。" },
  "freebuff-web": { preset: "freebuff", baseUrl: "https://freebuff.com", cookie: null, vendorKey: "freebuff", vendorName: "FreeBuff", special: "freebuff",
    hint: "需 FreeBuff authToken（填 token）和 agentId；不是广告或 Codebuff API Key。" },
  "perplexity-web": { preset: "perplexity-web", baseUrl: "https://www.perplexity.ai", cookie: "__Secure-next-auth.session-token", vendorKey: "perplexity", vendorName: "Perplexity", special: "perplexity_web",
    hint: "需消费端 session 和原会话 userAgent，可导入包装的 storageState；仅支持 turbo 策略的单轮纯文本，聚合后输出。独立于 Sonar/Search API，额度未知，尚待真实验收。" },
} as const;
export type SessionProduct = keyof typeof SESSION_PRODUCTS;

export function sessionProductForPreset(preset: unknown): SessionProduct | undefined {
  if (typeof preset !== "string") return undefined;
  const normalized = preset.trim().toLowerCase();
  if (["qwen-web", "qwen-webui", "qwen-webui-replay", "qwen-webui-replay-live"].includes(normalized)) return "qwen-web";
  return (Object.keys(SESSION_PRODUCTS) as SessionProduct[]).find((product) => SESSION_PRODUCTS[product].preset === normalized);
}
