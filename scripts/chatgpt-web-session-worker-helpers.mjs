export function selectNewAssistantTextFromValues(values, baselineValues) {
  const baseline = new Set(
    (Array.isArray(baselineValues) ? baselineValues : [])
      .map((value) => String(value || "").trim())
      .filter(Boolean),
  );
  const unique = Array.from(
    new Set(
      (Array.isArray(values) ? values : [])
        .map((value) => String(value || "").trim())
        .filter(Boolean),
    ),
  );
  const delta = unique.filter((text) => !baseline.has(text));
  return delta.length > 0 ? delta[delta.length - 1] : "";
}

export function isRelayAuthRecoveryState(pageState) {
  if (!pageState || typeof pageState !== "object") {
    return false;
  }
  if (pageState.hasEmail || pageState.hasPassword) {
    return true;
  }
  const href = String(pageState.href ?? "").toLowerCase();
  const title = String(pageState.title ?? "").toLowerCase();
  const body = String(pageState.bodyText ?? "").toLowerCase();
  const links = Array.isArray(pageState.authLinks)
    ? pageState.authLinks.map((value) => String(value).toLowerCase()).join("\n")
    : "";
  const combined = [href, title, body, links].join("\n");
  return [
    "auth.openai.com",
    "/auth/login_with",
    "/api/auth/error",
    "/auth/error",
    "email-verification",
    "your session has ended",
    "你的会话已结束",
    "log in to continue",
    "登录以继续",
    "continue to chatgpt",
    "log-in-or-create-account",
    "verification code",
    "check your inbox",
    "检查您的收件箱",
    "验证码",
  ].some((marker) => combined.includes(marker));
}
