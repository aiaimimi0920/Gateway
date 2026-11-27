// Bounded JSON admission before schema defaults: reject duplicate keys and lossy integer tokens.
// This intentionally accepts a stricter numeric subset than general JSON / ProtoJSON.
export function uniqueJson(bytes, { allowIntegerDecimals = false } = {}) {
  if (bytes.length > 512 * 1024) throw new Error("json_size_limit");
  const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  let at = 0;
  const fail = (reason = "syntax") => { throw new Error(`json_${reason}`); };
  const space = () => { while (/[ \t\r\n]/.test(text[at] ?? "") && at < text.length) at++; };
  const string = () => {
    const start = at++;
    while (at < text.length) {
      if (text[at] === "\\") { at += 2; continue; }
      if (text[at++] === '"') return JSON.parse(text.slice(start, at));
    }
    fail();
  };
  const value = (depth) => {
    if (depth > 64) fail("depth_limit");
    space();
    if (text[at] === '"') { string(); return; }
    if (text[at] === "{" || text[at] === "[") {
      const object = text[at++] === "{", end = object ? "}" : "]";
      const keys = new Set();
      space();
      if (text[at] === end) { at++; return; }
      for (;;) {
        space();
        if (object) {
          if (text[at] !== '"') fail();
          const key = string();
          if (keys.has(key)) fail("duplicate_key");
          keys.add(key);
          space();
          if (text[at++] !== ":") fail();
        }
        value(depth + 1);
        space();
        if (text[at] === end) { at++; return; }
        if (text[at++] !== ",") fail();
      }
    }
    const token = /^(?:true|false|null|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?)/.exec(text.slice(at))?.[0];
    if (!token) fail();
    if (!["true", "false", "null"].includes(token)) {
      const number = Number(token);
      if (!Number.isFinite(number)) fail("nonfinite_number");
      if (Number.isInteger(number) &&
          (!Number.isSafeInteger(number) || (!/^-?\d+$/.test(token) &&
            !(allowIntegerDecimals && /^-?\d+\.0+$/.test(token))))) fail("integer_form");
    }
    at += token.length;
  };
  value(0);
  space();
  if (at !== text.length) fail();
  return JSON.parse(text);
}
