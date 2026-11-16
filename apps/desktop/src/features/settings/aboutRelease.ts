import { z } from "zod";

export const projectUrl = "https://github.com/aiaimimi0920/Gateway";
export const releasesUrl = `${projectUrl}/releases`;
const releaseSchema = z.object({ tag_name: z.string().max(100), html_url: z.string().url(), body: z.string().max(512_000).nullable(), draft: z.boolean(), prerelease: z.boolean() });
export type PublicRelease = { version: string; url: string; notes: string };

/** Read public release metadata only; Gateway credentials must never leave the selected backend. */
export async function checkPublicRelease(signal: AbortSignal): Promise<PublicRelease | null> {
  const response = await fetch("https://api.github.com/repos/aiaimimi0920/Gateway/releases?per_page=30", {
    credentials: "omit", cache: "no-store", redirect: "error", signal,
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!response.ok) throw new Error(response.status === 403 || response.status === 429 ? "GitHub 请求受限，请稍后重试。" : `GitHub HTTP ${response.status}`);
  const reader = response.body?.getReader();
  if (!reader) throw new Error("发布信息为空。");
  const chunks: Uint8Array[] = []; let length = 0;
  try { for (;;) {
    const { value, done } = await reader.read(); if (done) break;
    length += value.byteLength;
    if (length > 2 * 1024 * 1024) throw new Error("发布信息过大。");
    chunks.push(value);
  } } finally { await reader.cancel(); reader.releaseLock(); }
  const bytes = new Uint8Array(length); let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
  const text = new TextDecoder().decode(bytes);
  const releases = z.array(releaseSchema).max(30).parse(JSON.parse(text));
  const release = releases.find(item => !item.draft && !item.prerelease && /^[vV]\d+\.\d+\.\d+$/.test(item.tag_name));
  if (!release) return null;
  if (!release.html_url.startsWith(`${releasesUrl}/tag/`)) throw new Error("发布链接不属于 Gateway 仓库。");
  return { version: release.tag_name, url: release.html_url, notes: release.body ?? "" };
}
