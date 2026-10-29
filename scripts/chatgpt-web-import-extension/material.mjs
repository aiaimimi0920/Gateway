// Pure whitelist conversion. JWT expiry is a hint, never proof of account ownership.
export function exportMaterial(session, cookies, now = Date.now()) {
  const text = (value, limit) => {
    if (typeof value !== "string" || !value.trim() || value.length > limit || /[\x00-\x1f\x7f]/.test(value)) throw new Error("invalid_material");
    return value;
  };
  const authToken = text(session.accessToken, 32768);
  const payload = authToken.split(".")[1];
  const expiry = JSON.parse(atob(payload.replaceAll("-", "+").replaceAll("_", "/"))).exp;
  if (!Number.isSafeInteger(expiry) || expiry * 1000 <= now || expiry * 1000 > 8640000000000000) throw new Error("expired_session");
  const scoped = cookies.filter(c => ["chatgpt.com", ".chatgpt.com"].includes(c.domain) && !c.partitionKey);
  const cookieHeader = scoped.map(c => {
    const name = text(c.name, 256);
    if (typeof c.value !== "string" || /[;\x00-\x1f\x7f]/.test(name + c.value) || name.includes("=")) throw new Error("invalid_material");
    return `${name}=${c.value}`;
  }).join("; ");
  const material = {
    authToken, expiresAt: new Date(expiry * 1000).toISOString(),
    cookieHeader: text(cookieHeader, 65536),
    deviceId: text(scoped.find(c => c.name === "oai-did")?.value, 256),
    userAgent: text(session.userAgent, 2048),
  };
  for (const field of ["language", "timezone", "chatgptPowDataBuild"]) {
    if (session[field] !== undefined) material[field] = text(session[field], 4096);
  }
  if (session.chatgptPowSources !== undefined) {
    if (!Array.isArray(session.chatgptPowSources) || session.chatgptPowSources.length > 32) throw new Error("invalid_material");
    material.chatgptPowSources = session.chatgptPowSources.map(value => text(value, 4096));
  }
  if (new TextEncoder().encode(JSON.stringify(material)).length > 128 * 1024) throw new Error("invalid_material");
  return material;
}
