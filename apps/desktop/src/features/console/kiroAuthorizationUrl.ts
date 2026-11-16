/** A server challenge is still untrusted at the browser navigation boundary. */
export function kiroAuthorizationUrl(value: string): string | null {
  if (!value || value.length > 4096 || !/^[\x21-\x7e]+$/.test(value)) return null;
  try {
    const url = new URL(value);
    if (url.protocol !== "https:" || url.port || url.username || url.password || value.includes("#")) return null;
    const regional = /^device\.sso\.[a-z]{2}(?:-[a-z]+)+-\d\.(?:amazonaws\.com|api\.aws)$/.test(url.hostname) && url.pathname === "/";
    const builder = url.hostname === "view.awsapps.com" && ["/start", "/start/"].includes(url.pathname);
    return regional || builder ? url.href : null;
  } catch { return null; }
}
