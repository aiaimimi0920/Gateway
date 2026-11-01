/** Completed output parts only; thought media never escape and PCM chunks merge in linear space. */
import { mediaBytes } from "./parts.mjs";

export function decodeFrames(root) {
  if (!Array.isArray(root?.[0]) || root[0].length > 16384) throw new Error("aistudio_http_response_invalid");
  const groups = []; let finish, usage, byteCount = 0, partCount = 0;
  for (const frame of root[0]) {
    if (!Array.isArray(frame)) throw new Error("aistudio_http_response_invalid");
    if (Array.isArray(frame[2]) && Number.isSafeInteger(frame[2][0]) && Number.isSafeInteger(frame[2][2])) {
      usage = { promptTokenCount: frame[2][0], candidatesTokenCount: frame[2][1] ?? 0, totalTokenCount: frame[2][2],
        cachedContentTokenCount: frame[2][3] ?? 0, thoughtsTokenCount: frame[2][9] ?? 0 };
    }
    if (!frame[0]?.length) continue;
    if (frame[0].length !== 1 || finish != null) throw new Error("aistudio_http_response_invalid");
    const candidate = frame[0][0], content = candidate[0];
    if (content != null) {
      if (!Array.isArray(content[0]) || content[1] !== "model") throw new Error("aistudio_http_response_invalid");
      for (const part of content[0]) {
        if (++partCount > 16384) throw new Error("aistudio_http_response_limit");
        if (part?.[12]) continue;
        const transcript = part?.[22]?.[0], texts = [part?.[1], ...(transcript !== part?.[1] ? [transcript] : [])];
        for (const text of texts.filter(value => typeof value === "string" && value)) {
          byteCount += Buffer.byteLength(text);
          if (groups.at(-1)?.kind === "text") groups.at(-1).chunks.push(text);
          else groups.push({ kind: "text", chunks: [text] });
        }
        if (part?.[2] != null) {
          const [mimeType, data] = part[2];
          if (typeof mimeType !== "string" || mimeType.length > 128 || !/^(?:image\/(?:png|jpeg|webp)|audio\/(?:wav|x-wav|mpeg|mp3|ogg|L16|l16|pcm)(?:;\s*[a-zA-Z0-9_-]+=[a-zA-Z0-9._-]+)*)$/.test(mimeType)) {
            throw new Error("aistudio_http_response_media_unsupported");
          }
          const bytes = mediaBytes(data, 20 * 1024 * 1024); byteCount += bytes.length;
          const last = groups.at(-1);
          if (/^audio\/(?:l16|pcm)/i.test(mimeType) && last?.mimeType === mimeType) last.chunks.push(bytes);
          else groups.push({ kind: "media", mimeType, chunks: [bytes] });
        }
        if (byteCount > 20 * 1024 * 1024) throw new Error("aistudio_http_response_limit");
      }
    }
    if (candidate[1] != null) finish = candidate[1];
  }
  if (finish == null) throw new Error("aistudio_http_response_incomplete");
  if (![1, 2].includes(finish) || !groups.length) throw new Error("aistudio_http_response_blocked");
  const parts = groups.map(group => group.kind === "text" ? { text: group.chunks.join("") }
    : { inlineData: { mimeType: group.mimeType, data: Buffer.concat(group.chunks).toString("base64") } });
  if (!parts.some(part => part.inlineData || part.text?.trim())) throw new Error("aistudio_http_response_blocked");
  return { candidates: [{ content: { role: "model", parts }, finishReason: finish === 1 ? "STOP" : "MAX_TOKENS" }],
    ...(usage ? { usageMetadata: usage } : {}) };
}
