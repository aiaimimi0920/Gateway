/** JSON pipe adapter. Errors are fixed codes; successful text goes only to the requesting Gateway. */
import { runHttpRequest } from "./aistudio-http/execution.mjs";
import { PIPE_INPUT_LIMIT } from "./aistudio-http/parts.mjs";

try {
  const chunks = []; let size = 0;
  for await (const chunk of process.stdin) {
    size += chunk.length;
    if (size > PIPE_INPUT_LIMIT) throw new Error("aistudio_http_input_limit");
    chunks.push(chunk);
  }
  const input = JSON.parse(Buffer.concat(chunks).toString("utf8"));
  const response = await runHttpRequest(input);
  process.stdout.write(`${JSON.stringify({ ok: true, response })}\n`);
} catch (error) {
  const code = /^aistudio_http_[a-z_]{1,64}$/.test(error.message) ? error.message : "aistudio_http_request_failed";
  process.stdout.write(`${JSON.stringify({ ok: false, code, status: [400, 401, 403, 429].includes(error.status) ? error.status : 503 })}\n`);
}
