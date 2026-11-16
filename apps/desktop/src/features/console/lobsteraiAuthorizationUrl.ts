// The fragment route is an explicit exception for this provider, never a general URL allowance.
export function validLobsterRedirect(value: string): boolean {
  if (!/^http:\/\/127\.0\.0\.1:[1-9][0-9]{0,4}\/auth\/callback$/.test(value)) return false;
  try { const url = new URL(value); return url.href === value && Number(url.port) > 0 && Number(url.port) <= 65535; } catch { return false; }
}
export function lobsteraiAuthorizationUrl(value: string): string | null {
  if (value.length > 8192 || /[\x00-\x20]/.test(value)) return null;
  try {
    const url = new URL(value);
    if (url.origin !== "https://lobsterai.youdao.com" || url.pathname !== "/portal" || url.username || url.password || url.search || !url.hash.startsWith("#/login?")) return null;
    const query = new URLSearchParams(url.hash.slice("#/login?".length));
    const keys = [...query.keys()];
    if (keys.length !== 3 || new Set(keys).size !== 3 || !keys.every((key) => ["source", "state", "redirect_uri"].includes(key))
      || query.get("source") !== "electron" || !/^[A-Za-z0-9_-]{43}$/.test(query.get("state") ?? "")
      || !validLobsterRedirect(query.get("redirect_uri") ?? "")) return null;
    return url.href;
  } catch { return null; }
}
