// Optional diagnostic vocabulary only. Matches are mentions, not proven causes or commands.
// Raw titles/details and unknown words never leave this bounded in-memory projection.
import { fields } from "./protobuf_wire.mjs";

const patterns = new Map([
  ["rate_limit", /\brate[\s-]*limit(?:ed|ing|s)?\b/i],
  ["usage_limit", /\b(?:usage|request|credit|quota)[\s-]+(?:limit|exhaust|exceed)/i],
  ["upgrade", /\b(?:upgrade|upgrading)\b/i],
  ["payment", /\b(?:payment|billing|paid plan|subscription)\b/i],
  ["free_plan", /\b(?:free|hobby)(?:[\s-]+(?:plan|tier|user|account))?\b/i],
  ["pro_plan", /\bpro(?:[\s-]+(?:plan|tier|user|account))?\b/i],
  ["model", /\bmodels?\b/i],
  ["authentication", /\b(?:log[\s-]*in|sign[\s-]*in|authentication|unauthenticated|token)\b/i],
  ["client_version", /\b(?:outdated|unsupported client|client version|update cursor|update required|version.{0,80}(?:no longer supported|not supported|unsupported)|update.{0,40}(?:latest|newer|supported) version)\b/i],
  ["unusual_activity", /\b(?:unusual|suspicious|abuse|blocked)\b/i],
  ["region", /\b(?:region|country|location)\b/i],
  ["capacity", /\b(?:capacity|overloaded|high demand|temporarily unavailable)\b/i],
  ["premium_plan", /\b(?:premium|ultra|pro\+)\b|高级套餐|付费套餐/i],
  ["limit_reached", /\b(?:(?:hit|reached|exceeded|exhausted).{0,60}(?:limit|quota|allowance)|(?:limit|quota|allowance).{0,30}(?:reached|exceeded|exhausted))\b|额度.{0,12}(?:耗尽|用完)|达到.{0,12}上限/i],
  ["too_many_requests", /\btoo many requests\b|请求.{0,12}(?:频繁|过多)/i],
  ["eligibility", /\b(?:eligible|eligibility|only available|only accessible|not available|not supported|not included)\b|仅.{0,12}(?:支持|可用)|不支持|无权使用/i],
  ["verification", /\b(?:verify|verified|verification|captcha)\b|验证|验证码/i],
  ["trial", /\btrial\b|试用/i],
  ["client_environment", /\b(?:client|environment|application|app|editor|cli|sdk|third.party)\b|客户端|第三方/i],
]);

// Ordered, fixed-vocabulary tokens retain negation/context without exporting arbitrary
// response strings. Non-vocabulary words, identifiers, numbers and secrets become "_".
const vocabulary = new Set(("a an the this that these those you your we our it its is are was were be been " +
  "can cannot could may must need needs require requires required not no only all any for from in on at with without " +
  "to of and or but because if please try again later now currently has have had do does did support supported supports " +
  "available unavailable enabled disabled eligible eligibility access accessible allowed allow forbidden denied " +
  "account accounts user users free hobby pro premium ultra paid plan plans subscription subscriptions trial " +
  "usage quota limit limits limited reached hit exceed exceeded exhausted remaining requests request rate slow fast " +
  "model models feature features mode auto default agent agents client clients app application editor environment " +
  "cli sdk api third party official payment billing upgrade continue update version outdated login sign token " +
  "authentication verify verified verification phone email maximum number daily monthly temporary temporarily " +
  "wait restricted restriction restrictions region country service error failed blocked pricing included left " +
  "enough credits credit balance longer latest newer").split(" "));
export const safeMessageToken = (value) => vocabulary.has(value) ? value : "_";

export const safeMessageSignal = (value) => patterns.has(value) ? value : "unrecognized";
export const noMessageSignals = (status = "absent") => ({
  remoteCursorMessageSignals: [], remoteCursorMessageStatus: status, remoteCursorMessageTokens: [],
});

function summarize(texts) {
  if (!texts.some((text) => text.length)) return noMessageSignals();
  // Mask complete identifiers before tokenization; never turn an email/URL/JWT into words.
  const text = texts.join("\n").replace(/\S*(?:@|:\/\/)\S*|\b[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b/g, "_");
  const signals = [...patterns].filter(([, pattern]) => pattern.test(text)).map(([key]) => key);
  const normalized = text.toLowerCase().replace(/\b(can't|cannot|isn't|aren't|doesn't|don't|you've|you're)\b/g,
    (word) => ({ "can't": "cannot", cannot: "cannot", "isn't": "is not", "aren't": "are not",
      "doesn't": "does not", "don't": "do not", "you've": "you have", "you're": "you are" })[word]);
  const tokens = (normalized.match(/[a-z0-9_+/-]+(?:'[a-z]+)?|[^\s.,;:!?()[\]{}"“”]+/g) ?? [])
    .slice(0, 96).map(safeMessageToken);
  return { remoteCursorMessageSignals: signals,
    remoteCursorMessageStatus: signals.length ? "recognized" : "unclassified", remoteCursorMessageTokens: tokens };
}

export function connectMessageSignals(text) {
  let result;
  if (text === undefined || text === "") result = noMessageSignals();
  else if (typeof text !== "string") result = noMessageSignals("invalid");
  else if (Buffer.byteLength(text) > 4096) result = noMessageSignals("too_large");
  else result = summarize([text]);
  return { remoteConnectMessageSignals: result.remoteCursorMessageSignals,
    remoteConnectMessageStatus: result.remoteCursorMessageStatus,
    remoteConnectMessageTokens: result.remoteCursorMessageTokens };
}

export function cursorMessageSignals(bytes) {
  if (bytes === undefined) return noMessageSignals();
  try {
    const items = fields(bytes), texts = [];
    // Pinned first-party CustomErrorDetails: #1 title and #2 detail.
    for (const tag of [1, 2]) {
      const values = items.filter((item) => item.tag === tag);
      if (values.length > 1 || values.some((item) => item.wire !== 2)) return noMessageSignals("invalid");
      if (!values.length) continue;
      if (values[0].value.length > 4096) return noMessageSignals("too_large");
      texts.push(new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(values[0].value));
    }
    return summarize(texts);
  } catch { return noMessageSignals("invalid"); }
}
