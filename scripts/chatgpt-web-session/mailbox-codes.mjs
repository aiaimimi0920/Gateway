import { normalizeString } from "./configuration.mjs";

export function extractMailboxCodeMarker(codeObject) {
  if (!codeObject || typeof codeObject !== "object") {
    return 0;
  }
  return Math.max(
    parseMailTimestamp(codeObject.observedAt),
    parseMailTimestamp(codeObject.receivedAt),
    Number(codeObject.messageId ?? 0) || 0,
  );
}

export function selectOpenAiVerificationCode(codeObject) {
  if (!codeObject || typeof codeObject !== "object") {
    return "";
  }
  const direct = extractSixDigitCode(codeObject.extractedCode ?? codeObject.code);
  if (direct) {
    return direct;
  }
  if (Array.isArray(codeObject.extractedCandidates)) {
    for (const item of codeObject.extractedCandidates) {
      const candidate = extractSixDigitCode(item);
      if (candidate) {
        return candidate;
      }
    }
  }
  return extractOpenAiCodeFromMessage(codeObject);
}

export function extractOpenAiCodeFromMessage(message) {
  if (!message || typeof message !== "object") {
    return "";
  }
  for (const key of ["subject", "textBody", "htmlBody"]) {
    let text = String(message[key] ?? "");
    if (key === "htmlBody") {
      text = text.replace(/<[^>]+>/g, " ");
    }
    text = text.replace(/https?:\/\/\S+/gi, " ");
    text = text.replace(/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/gi, " ");
    text = text.replace(/\s+/g, " ").trim();
    const contextual = text.match(
      /(?:verification\s*code|verify\s*code|security\s*code|one[-\s]*time\s*(?:pass)?code|login\s*code|sign[\s-]*in\s*code|confirmation\s*code|email\s*code|otp|passcode|验证码|校验码|动态码|动态密码|口令|代码为|代码是|enter\s+this\s+temporary\s+verification\s+code)[^0-9]{0,80}(\d{6})(?!\d)/i,
    );
    if (contextual?.[1]) {
      return contextual[1];
    }
  }
  for (const key of ["subject", "textBody", "htmlBody"]) {
    let text = String(message[key] ?? "");
    if (key === "htmlBody") {
      text = text.replace(/<[^>]+>/g, " ");
    }
    const candidate = extractSixDigitCode(text);
    if (candidate) {
      return candidate;
    }
  }
  return "";
}

export function extractSixDigitCode(value) {
  const match = String(value ?? "").match(/(?<!\d)(\d{6})(?!\d)/);
  return match?.[1] ?? "";
}

export function parseMailTimestamp(value) {
  const text = normalizeString(value);
  if (!text) {
    return 0;
  }
  const epoch = Date.parse(String(text).replace("Z", "+00:00"));
  return Number.isFinite(epoch) ? epoch : 0;
}
