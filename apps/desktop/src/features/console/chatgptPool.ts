export const CHATGPT_POOL_CATEGORIES = [
  { id: "free", label: "free" },
  { id: "plus", label: "plus" },
  { id: "pro", label: "pro" },
  { id: "pro20x", label: "pro20x" },
  { id: "team-mother", label: "team mother" },
  { id: "team-child", label: "team child" },
] as const;

export function isChatgptPool(preset: string | null, profile: string | null): boolean {
  return preset === "chatgpt-codex-oauth-official-api" || profile === "chatgpt_codex_backend";
}
