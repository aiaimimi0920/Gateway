/** Bounded inline media admission: never fetch caller URLs or resolve local paths/file IDs. */
import { dataPart, inputPart } from "./input-part.mjs";
const INPUT_MIMES = new Set(["image/png", "image/jpeg", "image/webp", "image/gif", "audio/wav", "audio/x-wav",
  "audio/mpeg", "audio/mp3", "audio/ogg", "audio/flac", "audio/aac", "audio/mp4", "audio/webm",
  "video/mp4", "video/webm", "video/quicktime", "video/mpeg", "application/pdf"]);
export const MEDIA_INPUT_LIMIT = 6 * 1024 * 1024;
export const PIPE_INPUT_LIMIT = 9 * 1024 * 1024;
export const RESPONSE_LIMIT = 32 * 1024 * 1024;

export function mediaBytes(data, limit = MEDIA_INPUT_LIMIT) {
  if (typeof data !== "string" || data.length > Math.ceil(limit / 3) * 4) throw new Error("aistudio_http_media_limit");
  if (!data || data.length % 4 || !/^[A-Za-z0-9+/]*={0,2}$/.test(data)) {
    throw new Error("aistudio_http_media_base64_invalid");
  }
  const bytes = Buffer.from(data, "base64");
  if (!bytes.length || bytes.length > limit) throw new Error("aistudio_http_media_limit");
  if (bytes.toString("base64") !== data) throw new Error("aistudio_http_media_base64_invalid");
  return bytes;
}

export function encodeParts(parts, budget, textOnly = false) {
  if (!Array.isArray(parts) || !parts.length || parts.length > 256) throw new Error("aistudio_http_parts_invalid");
  return parts.map(part => {
    part = inputPart(part);
    if (!part || typeof part !== "object" || Array.isArray(part) || Object.keys(part).length !== 1) throw new Error("aistudio_http_parts_invalid");
    if (typeof part.text === "string") return [null, part.text];
    if (textOnly) throw new Error("aistudio_http_system_text_only");
    let inline = part.inlineData;
    if (part.fileData) {
      const file = part.fileData;
      if (Object.keys(file).some(key => !["fileUri", "mimeType"].includes(key))) throw new Error("aistudio_http_parts_invalid");
      // The data URI is authoritative; the shared Gemini packer guesses PNG from URL suffixes.
      inline = dataPart(file.fileUri).inlineData;
    }
    if (!inline || !INPUT_MIMES.has(inline.mimeType) || Object.keys(inline).some(key => !["mimeType", "data"].includes(key))) {
      throw new Error("aistudio_http_media_type_unsupported");
    }
    budget.bytes += mediaBytes(inline.data).length;
    if (budget.bytes > MEDIA_INPUT_LIMIT || ++budget.parts > 16) throw new Error("aistudio_http_media_limit");
    return [null, null, [inline.mimeType, inline.data]];
  });
}
