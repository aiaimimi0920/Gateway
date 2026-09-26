export function safeParseJson(text) {
  try {
    return JSON.parse(String(text ?? ""));
  } catch {
    return null;
  }
}
