export type ProviderVerificationStatus = "verified" | "failed" | "blocked" | "not-tested";

export type ProviderVerificationRecord = {
  status: ProviderVerificationStatus;
  families: string[];
  checkedAt: string | null;
  evidenceRef: string | null;
  note: string;
};

// Secret-free live evidence index. Evidence refs point to local canary artifact
// folders; response bodies, credentials, and upstream URLs are never embedded.
export const PROVIDER_VERIFICATION_REGISTRY: Record<string, ProviderVerificationRecord> = {
  linkup: {
    status: "verified",
    families: ["search", "fetch", "research", "credits_balance"],
    checkedAt: "2026-08-12T11:44:00+08:00",
    evidenceRef: "live-provider-canary-linkup-20260812-r3",
    note: "Search, fetch, research completion, and balance passed semantic route proof.",
  },
  tavily: {
    status: "verified",
    families: ["search"],
    checkedAt: "2026-08-12T11:48:00+08:00",
    evidenceRef: "live-provider-canary-search-20260812",
    note: "Search returned the Paris answer with matching route proof.",
  },
  you: {
    status: "verified",
    families: ["search"],
    checkedAt: "2026-08-12T11:48:00+08:00",
    evidenceRef: "live-provider-canary-search-20260812",
    note: "Search returned the Paris answer with matching route proof.",
  },
  exa: {
    status: "verified",
    families: ["search", "fetch"],
    checkedAt: "2026-08-12T11:51:00+08:00",
    evidenceRef: "live-provider-canary-fetch-20260812",
    note: "Search and fetch passed semantic route proof.",
  },
  jina: {
    status: "verified",
    families: ["search", "reader"],
    checkedAt: "2026-08-12T11:51:00+08:00",
    evidenceRef: "live-provider-canary-fetch-20260812",
    note: "Search and reader fetch passed semantic route proof.",
  },
  "jina-search": {
    status: "verified",
    families: ["search"],
    checkedAt: "2026-08-12T11:48:00+08:00",
    evidenceRef: "live-provider-canary-search-20260812",
    note: "Search returned the Paris answer with matching route proof.",
  },
  "jina-reader": {
    status: "verified",
    families: ["reader"],
    checkedAt: "2026-08-12T11:51:00+08:00",
    evidenceRef: "live-provider-canary-fetch-20260812",
    note: "Reader fetch returned the requested page with matching route proof.",
  },
  openrouter: {
    status: "verified",
    families: ["conversation"],
    checkedAt: "2026-08-12T12:02:00+08:00",
    evidenceRef: "live-provider-canary-text-unique-20260812",
    note: "Conversation returned the Paris answer with matching route proof.",
  },
  "muyuan-openai": {
    status: "failed",
    families: [],
    checkedAt: "2026-08-12T12:43:07+08:00",
    evidenceRef: "live-provider-canary-unmanifested-text-20260812-r3",
    note: "Upstream rejected the configured credential with 401.",
  },
  poe: {
    status: "failed",
    families: [],
    checkedAt: "2026-08-12T12:43:10+08:00",
    evidenceRef: "live-provider-canary-unmanifested-text-20260812-r3",
    note: "Route proof reached the current model, but the account returned 402 insufficient quota.",
  },
  longcat: {
    status: "failed",
    families: [],
    checkedAt: "2026-08-12T12:43:10+08:00",
    evidenceRef: "live-provider-canary-unmanifested-text-20260812-r3",
    note: "Current model reached LongCat but the account returned 402 insufficient quota.",
  },
  nvidia: {
    status: "failed",
    families: [],
    checkedAt: "2026-08-12T12:13:00+08:00",
    evidenceRef: "live-provider-canary-text-unique-20260812",
    note: "The canary received a 404; route configuration needs a fresh upstream check.",
  },
  "qwen-dashscope-openai": {
    status: "failed",
    families: [],
    checkedAt: "2026-08-12T12:09:00+08:00",
    evidenceRef: "live-provider-canary-text-unique-20260812",
    note: "DashScope rejected the configured API key with 401.",
  },
  "qwen-coding-plan-openai": {
    status: "failed",
    families: [],
    checkedAt: "2026-08-12T12:10:00+08:00",
    evidenceRef: "live-provider-canary-qwen-coding-20260812",
    note: "Coding Plan OpenAI endpoint rejected the access token with 401.",
  },
  "qwen-coding-plan-anthropic": {
    status: "failed",
    families: [],
    checkedAt: "2026-08-12T12:10:00+08:00",
    evidenceRef: "live-provider-canary-qwen-coding-20260812",
    note: "Coding Plan Anthropic endpoint rejected the access token with 401.",
  },
  "qwen-web-chat": {
    status: "blocked",
    families: [],
    checkedAt: "2026-08-12T12:13:14+08:00",
    evidenceRef: "live-provider-canary-qwen-web-20260812",
    note: "Shared model routing selected the official Qwen credential before the compiled-out Web line.",
  },
  codex: {
    status: "failed",
    families: [],
    checkedAt: "2026-08-12T12:05:00+08:00",
    evidenceRef: "live-provider-canary-text-unique-20260812",
    note: "OAuth credential was rejected with 401.",
  },
  accio: {
    status: "failed",
    families: [],
    checkedAt: "2026-08-12T12:05:00+08:00",
    evidenceRef: "live-provider-canary-text-unique-20260812",
    note: "Upstream returned an unauthorized error.",
  },
  xfyun: {
    status: "failed",
    families: [],
    checkedAt: "2026-08-12T12:06:00+08:00",
    evidenceRef: "live-provider-canary-text-unique-20260812",
    note: "MaaS credentials were rejected with 403.",
  },
  "websearchapi": {
    status: "failed",
    families: [],
    checkedAt: "2026-08-12T11:49:00+08:00",
    evidenceRef: "live-provider-canary-search-20260812",
    note: "Upstream rejected the configured credential with 401.",
  },
  chataibot: {
    status: "blocked",
    families: [],
    checkedAt: "2026-08-12T13:02:35+08:00",
    evidenceRef: "live-provider-canary-media-sync-20260812",
    note: "Image generation was blocked because the saved token is invalid.",
  },
  lumalabs: {
    status: "blocked",
    families: [],
    checkedAt: "2026-08-12T13:02:38+08:00",
    evidenceRef: "live-provider-canary-media-sync-20260812",
    note: "Image, music, and video probes redirected to login; the browser session must be refreshed.",
  },
  producer: {
    status: "blocked",
    families: [],
    checkedAt: "2026-08-12T13:02:40+08:00",
    evidenceRef: "live-provider-canary-media-sync-20260812",
    note: "Image and music returned 401; video also requires a successful music clip id.",
  },
  suno: {
    status: "blocked",
    families: [],
    checkedAt: "2026-08-12T13:03:10+08:00",
    evidenceRef: "live-provider-canary-media-long-20260812",
    note: "Music generation requires a raw authenticated browser Cookie credential.",
  },
  udio: {
    status: "blocked",
    families: [],
    checkedAt: "2026-08-12T13:03:25+08:00",
    evidenceRef: "live-provider-canary-media-long-20260812",
    note: "Music generation was rejected because the browser session is not authenticated.",
  },
};

export function providerVerificationFor(
  providerId: string,
  vendorKey?: string | null,
): ProviderVerificationRecord {
  return (
    PROVIDER_VERIFICATION_REGISTRY[providerId] ??
    (vendorKey ? PROVIDER_VERIFICATION_REGISTRY[vendorKey] : undefined) ?? {
      status: "not-tested",
      families: [],
      checkedAt: null,
      evidenceRef: null,
      note: "No live semantic canary evidence has been recorded.",
    }
  );
}
