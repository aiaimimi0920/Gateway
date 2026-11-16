// Bind only the model of one basic native Create request. Preserve page auth/challenge material.
import { createError } from "./pure.mjs";
const mismatch = () => createError(409, "suno_browser_request_mismatch",
  "Suno native request is not the confirmed basic task; details omitted. Do not resubmit automatically.");
const ambiguous = () => createError(409, "suno_browser_ambiguous_generation",
  "Suno native submission is ambiguous; do not resubmit automatically.");
const object = value => Boolean(value && typeof value === "object" && !Array.isArray(value));
const empty = value => value === undefined || value === null || value === "";
const nullable = ["user_uploaded_images_b64", "cover_clip_id", "cover_start_s", "cover_end_s",
  "persona_id", "artist_clip_id", "artist_start_s", "artist_end_s", "continue_clip_id",
  "continued_aligned_prompt", "continue_at", "title", "tags", "negative_tags"];
const rootKeys = new Set(["mv", "token", "generation_type", "prompt", "gpt_description_prompt",
  "make_instrumental", "metadata", "override_fields", "transaction_uuid", "token_provider", ...nullable]);
const metadataKeys = new Set(["web_client_pathname", "is_max_mode", "is_mumble", "create_mode",
  "user_tier", "create_session_token", "disable_volume_normalization", "control_sliders", "lyrics_model"]);
export function boundNativeModel(body) {
  if (!object(body) || !["chirp-auk", "chirp-auk-turbo"].includes(body.model) ||
      Object.keys(body).some(key => !["model", "prompt", "waitCompletion"].includes(key))) throw mismatch();
  return body.model;
}
export function bindNativeBody(text, model, prompt) {
  if (typeof text !== "string" || Buffer.byteLength(text) > 128 * 1024) throw mismatch();
  let body;
  try { body = JSON.parse(text); } catch { throw mismatch(); }
  const meta = body?.metadata;
  if (!object(body) || !object(meta) || Object.keys(body).some(key => !rootKeys.has(key)) ||
      Object.keys(meta).some(key => !metadataKeys.has(key)) || meta.create_mode !== "simple" ||
      body.generation_type !== "TEXT" || body.make_instrumental !== false ||
      typeof body.mv !== "string" || !body.mv || Buffer.byteLength(body.mv) > 128 ||
      nullable.some(key => !empty(body[key])) ||
      ["is_max_mode", "is_mumble", "disable_volume_normalization"].some(key => meta[key] !== undefined && meta[key] !== false) ||
      (!empty(meta.lyrics_model) && meta.lyrics_model !== "default") ||
      (!empty(meta.control_sliders) && (!object(meta.control_sliders) || Object.values(meta.control_sliders).some(value => !empty(value)))) ||
      (body.override_fields !== undefined && (!Array.isArray(body.override_fields) || body.override_fields.length !== 0))) throw mismatch();
  const matchesPrompt = (body.prompt === prompt && empty(body.gpt_description_prompt)) ||
    (body.gpt_description_prompt === prompt && empty(body.prompt));
  if (!matchesPrompt) throw mismatch();
  // This is a model selection, not a body/auth reconstruction. Unknown operation shapes fail closed.
  body.mv = model;
  return JSON.stringify(body);
}
export function createNativeModelGate(page, baseUrl, model, prompt, onFailure) {
  const origin = new URL(baseUrl).origin;
  const matches = value => {
    try { const url = new URL(value); return url.origin === origin && url.pathname.replace(/\/+$/, "") === "/api/generate/v2-web"; }
    catch { return false; }
  };
  let armed = false, stopped = false, seen = false, admitted = false, failure, removal;
  const active = new Set();
  const fail = reason => { failure ??= reason; onFailure(failure); };
  const handle = async route => {
    const request = route.request();
    if (request.method().toUpperCase() !== "POST") return route.fallback();
    if (stopped || !armed || seen) { fail(ambiguous()); return route.abort("blockedbyclient"); }
    seen = true; // Reserve synchronously, including a rejected first POST.
    const postData = bindNativeBody(request.postData(), model, prompt);
    admitted = true;
    // Final continuation prevents later page handlers from changing the bound body.
    await route.continue({ postData });
  };
  const handler = route => {
    const work = handle(route).catch(async () => {
      fail(mismatch());
      try { await route.abort("blockedbyclient"); } catch { /* The failure remains authoritative. */ }
    });
    active.add(work); void work.then(() => active.delete(work)); return work;
  };
  const remove = () => (removal ??= page.unroute(matches, handler));
  const ready = page.route(matches, handler).then(() => stopped ? remove() : undefined);
  void ready.catch(() => undefined);
  return {
    ready,
    arm() { if (failure) throw failure; if (stopped) throw ambiguous(); armed = true; },
    admitted: () => admitted && !failure,
    check() { if (failure) throw failure; },
    async stop() {
      stopped = true;
      let timer;
      try {
        await Promise.race([(async () => { await ready; await remove(); await Promise.allSettled([...active]); })(),
          new Promise((_, reject) => { timer = setTimeout(() => reject(mismatch()), 5000); })]);
      } finally { clearTimeout(timer); }
      if (failure) throw failure;
    },
  };
}
