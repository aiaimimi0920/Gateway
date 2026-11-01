/** Normalize caller-owned inline media only. Names are metadata, never filesystem paths. */
function fields(value, names) {
  if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).some(key => !names.includes(key))) {
    throw new Error("aistudio_http_parts_invalid");
  }
}

export function dataPart(uri) {
  const match = typeof uri === "string" && /^data:([a-z0-9.+/-]+);base64,([A-Za-z0-9+/=]+)$/.exec(uri);
  if (!match) throw new Error("aistudio_http_remote_media_unsupported");
  return { inlineData: { mimeType: match[1], data: match[2] } };
}

export function inputPart(part) {
  if (part?.type === "input_audio") {
    fields(part, ["type", "input_audio"]); fields(part.input_audio, ["format", "data"]);
    const mime = { wav: "audio/wav", mp3: "audio/mpeg", flac: "audio/flac", ogg: "audio/ogg", m4a: "audio/mp4", webm: "audio/webm" }[part.input_audio.format];
    if (!mime) throw new Error("aistudio_http_media_type_unsupported");
    return { inlineData: { mimeType: mime, data: part.input_audio.data } };
  }
  if (["file", "input_file"].includes(part?.type)) {
    fields(part, part.type === "file" ? ["type", "file"] : ["type", "filename", "file_data"]);
    const file = part.type === "file" ? part.file : part;
    fields(file, part.type === "file" ? ["filename", "file_data"] : ["type", "filename", "file_data"]);
    if (file.filename != null && (typeof file.filename !== "string" || file.filename.length > 1024)) throw new Error("aistudio_http_parts_invalid");
    return dataPart(file.file_data);
  }
  if (part?.inline_data) {
    fields(part, ["inline_data"]); fields(part.inline_data, ["mime_type", "data"]);
    return { inlineData: { mimeType: part.inline_data.mime_type, data: part.inline_data.data } };
  }
  if (part?.file_data) {
    fields(part, ["file_data"]); fields(part.file_data, ["mime_type", "file_uri"]);
    return dataPart(part.file_data.file_uri);
  }
  return part;
}
