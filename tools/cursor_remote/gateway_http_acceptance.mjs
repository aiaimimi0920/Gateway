// One POST only. Timeout, redirects and unknown delivery never trigger a retry.
import { safeCursorErrorCode } from "./gateway_acceptance.mjs";

export async function httpOnce(base, token, expected, send = fetch) {
  const response = await send(`${base.replace(/\/$/, "")}/v1/responses`, {
    method: "POST", redirect: "error", signal: AbortSignal.timeout(130000),
    headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: JSON.stringify({ model: "cursor-personal", store: false, stream: false,
      input: `This is a single text connectivity test. Reply exactly: ${expected}` }),
  });
  const reader = response.body.getReader(), chunks = [];
  let size = 0;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > 128 * 1024) throw new Error("acceptance_response_too_large");
      chunks.push(Buffer.from(value));
    }
  } finally {
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
  const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
  const text = (Array.isArray(body.output) ? body.output : [])
    .flatMap((item) => Array.isArray(item.content) ? item.content : [])
    .filter((item) => item.type === "output_text").map((item) => item.text).join("");
  return { httpStatus: response.status, completed: body.status === "completed",
    expectedReplyObserved: text.trim() === expected, responseBytes: size,
    errorCode: safeCursorErrorCode(body.error?.code) };
}
