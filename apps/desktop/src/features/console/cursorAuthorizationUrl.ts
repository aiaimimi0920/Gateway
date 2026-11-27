// The link can carry only the public PKCE challenge, never a token or a caller-selected redirect.
export function cursorAuthorizationUrl(value: string): string | null {
  if (value.length > 1024 || /[\x00-\x20]/.test(value)) return null;
  try {
    const url = new URL(value), params = url.searchParams;
    if (url.origin !== "https://www.cursor.com" || url.pathname !== "/loginDeepControl" ||
        url.username || url.password || url.hash || value.includes("#") ||
        [...params.keys()].sort().join(",") !== "challenge,mode,redirectTarget,uuid" ||
        params.get("mode") !== "login" || params.get("redirectTarget") !== "cli" ||
        !/^[A-Za-z0-9_-]{43}$/.test(params.get("challenge") ?? "") ||
        !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(params.get("uuid") ?? "")) return null;
    return url.href;
  } catch { return null; }
}
