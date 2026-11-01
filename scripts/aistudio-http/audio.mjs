/** AI Studio emits little-endian PCM despite its audio/l16 label. No browser or disk media files. */
import { spawn } from "node:child_process";
import { mediaBytes } from "./parts.mjs";
import { runtimeExecutable } from "./runtime-path.mjs";

export function audioFormat(format, encoder = process.env.AISTUDIO_FFMPEG_BIN) {
  if (format == null) return;
  if (!["wav", "pcm", "mp3"].includes(format)) throw new Error("aistudio_http_audio_format_unsupported");
  if (format === "mp3") return runtimeExecutable(process.platform === "win32" ? "ffmpeg.exe" : "ffmpeg", encoder,
    "aistudio_http_audio_encoder_missing", { searchPath: process.env.PATH });
  return encoder;
}

async function encodeMp3(bytes, rate, channels, executable) {
  const child = spawn(executable, ["-hide_banner", "-loglevel", "error", "-nostdin", "-protocol_whitelist", "pipe",
    "-f", "s16le", "-ar", String(rate), "-ac", String(channels), "-i", "pipe:0", "-f", "mp3", "-codec:a", "libmp3lame", "-b:a", "128k", "pipe:1"],
  { shell: false, windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
  const chunks = []; let size = 0, exceeded = false;
  const timer = setTimeout(() => child.kill(), 15000);
  child.stdout.on("data", chunk => { size += chunk.length; if (size > 8 * 1024 * 1024) { exceeded = true; child.kill(); } else chunks.push(chunk); });
  child.stderr.resume(); child.stdin.on("error", () => {});
  const closed = new Promise((resolve, reject) => { child.once("error", reject); child.once("close", resolve); });
  child.stdin.end(bytes);
  try {
    if (await closed !== 0 || exceeded || !size) throw new Error("aistudio_http_audio_encode_failed");
    return Buffer.concat(chunks);
  } finally { clearTimeout(timer); }
}

export async function formatAudio(response, format, encoder = audioFormat(format)) {
  if (format == null) return response;
  if (response.candidates?.[0]?.finishReason !== "STOP") throw new Error("aistudio_http_audio_incomplete");
  const parts = response.candidates?.[0]?.content?.parts;
  if (parts?.length !== 1 || !parts[0].inlineData) throw new Error("aistudio_http_audio_response_invalid");
  const media = parts[0].inlineData, bytes = mediaBytes(media.data, 20 * 1024 * 1024);
  const mime = media.mimeType.toLowerCase();
  if (!/^audio\/(?:l16|pcm)(?:;|$)/.test(mime)) {
    if (format === "wav" && mime === "audio/wav" || format === "mp3" && mime === "audio/mpeg") return response;
    throw new Error("aistudio_http_audio_response_invalid");
  }
  const rate = Number(/;\s*rate=(\d+)/.exec(mime)?.[1]), channels = Number(/;\s*channels=(\d+)/.exec(mime)?.[1] ?? 1);
  if (!Number.isInteger(rate) || rate < 8000 || rate > 96000 || ![1, 2].includes(channels) || bytes.length % (channels * 2)) throw new Error("aistudio_http_audio_response_invalid");
  let output = bytes, mimeType = `audio/pcm;rate=${rate};channels=${channels}`;
  if (format === "wav") {
    const header = Buffer.alloc(44); header.write("RIFF"); header.writeUInt32LE(36 + bytes.length, 4); header.write("WAVEfmt ", 8);
    header.writeUInt32LE(16, 16); header.writeUInt16LE(1, 20); header.writeUInt16LE(channels, 22); header.writeUInt32LE(rate, 24);
    header.writeUInt32LE(rate * channels * 2, 28); header.writeUInt16LE(channels * 2, 32); header.writeUInt16LE(16, 34);
    header.write("data", 36); header.writeUInt32LE(bytes.length, 40); output = Buffer.concat([header, bytes]); mimeType = "audio/wav";
  } else if (format === "mp3") { output = await encodeMp3(bytes, rate, channels, encoder); mimeType = "audio/mpeg"; }
  parts[0] = { inlineData: { mimeType, data: output.toString("base64") } };
  return response;
}
