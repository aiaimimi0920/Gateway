/** Playground generation slots; unimplemented native controls fail before any network request. */
export function generationConfig(source = {}) {
  if (!source || typeof source !== "object" || Array.isArray(source)) throw new Error("aistudio_http_config_invalid");
  if (source.candidateCount != null && source.candidateCount !== 1) throw new Error("aistudio_http_option_unsupported");
  const config = Array(14).fill(null), modalities = source.responseModalities;
  if (modalities != null) {
    if (!Array.isArray(modalities) || !modalities.length || modalities.length > 3
      || modalities.some(m => !["TEXT", "IMAGE", "AUDIO"].includes(m))
      || modalities.includes("AUDIO") && modalities.length !== 1) throw new Error("aistudio_http_config_invalid");
    config[14] = modalities.includes("AUDIO") ? [3] : [...(modalities.includes("IMAGE") ? [2] : []), ...(modalities.includes("TEXT") ? [1] : [])];
  }
  const image = modalities?.includes("IMAGE"), audio = modalities?.includes("AUDIO");
  config[3] = source.maxOutputTokens ?? (audio ? null : 8192);
  if (config[3] != null && (!Number.isInteger(config[3]) || config[3] < 1 || config[3] > 32768)) throw new Error("aistudio_http_output_limit");
  for (const [name, slot, maximum] of [["temperature", 4, 2], ["topP", 5, 1], ["topK", 6, 100]]) {
    if (source[name] != null) {
      if (!Number.isFinite(source[name]) || source[name] < 0 || source[name] > maximum) throw new Error("aistudio_http_config_invalid");
      config[slot] = source[name];
    }
  }
  if (source.stopSequences != null) {
    if (!Array.isArray(source.stopSequences) || source.stopSequences.length > 16
      || source.stopSequences.some(s => typeof s !== "string" || s.length > 256)) throw new Error("aistudio_http_config_invalid");
    config[1] = source.stopSequences;
  }
  if (source.responseMimeType != null) {
    if (!["text/plain", "application/json"].includes(source.responseMimeType)) throw new Error("aistudio_http_config_invalid");
    config[7] = source.responseMimeType;
  }
  if (source.responseSchema || source.responseJsonSchema) throw new Error("aistudio_http_option_unsupported");
  config[13] = image ? null : 1;
  if (source.imageConfig) {
    const { aspectRatio, imageSize } = source.imageConfig;
    if (!image || Object.keys(source.imageConfig).some(k => !["aspectRatio", "imageSize"].includes(k))
      || aspectRatio != null && !["1:1", "2:3", "3:2", "3:4", "4:3", "4:5", "5:4", "9:16", "16:9", "21:9"].includes(aspectRatio)
      || imageSize != null && !["512", "1K", "2K", "4K"].includes(imageSize)) throw new Error("aistudio_http_config_invalid");
    config[26] = [aspectRatio ?? null, imageSize ?? null];
  }
  if (source.speechConfig) {
    const speech = source.speechConfig, voice = speech.voiceConfig?.prebuiltVoiceConfig?.voiceName;
    if (!audio || Object.keys(speech).some(k => k !== "voiceConfig") || typeof voice !== "string"
      || !/^[A-Za-z][A-Za-z0-9_-]{0,63}$/.test(voice)) throw new Error("aistudio_http_speech_config_unsupported");
    config[15] = [[[voice]]];
  }
  return config;
}
