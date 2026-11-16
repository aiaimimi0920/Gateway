// Per-call gate for the borrowed page's legacy POST. It never rewrites native body or auth.
const keys = ["model_type", "modelType", "upstream_model_type", "upstreamModelType"];
const mismatch = () => Object.assign(new Error("Udio native request does not match the routed model; body omitted. Do not resubmit automatically."),
  { status: 409, code: "udio_browser_model_mismatch" });
const ambiguous = () => Object.assign(new Error("Udio native submission is ambiguous; do not resubmit automatically."),
  { status: 409, code: "udio_browser_ambiguous_generation" });
export function boundNativeModel(body) {
  const model = body?.gen_params?.model_type;
  if (typeof model !== "string" || !model || Buffer.byteLength(model) > 128 || /[\p{White_Space}\p{Cc}]/u.test(model)) throw mismatch();
  return model;
}
function matchesBody(text, expected) {
  if (typeof text !== "string" || Buffer.byteLength(text) > 1024 * 1024) return false;
  let body;
  try { body = JSON.parse(text); } catch { return false; }
  const params = body?.gen_params;
  if (!params || typeof params !== "object" || Array.isArray(params) || params.model_type !== expected) return false;
  return [body, params].every(object => keys.every(key => !Object.hasOwn(object, key) || object[key] === expected));
}
export function createNativeModelGate(page, matches, expected, fail) {
  let armed = false, stopped = false, seen = false, admitted = false, removal;
  const active = new Set();
  const handle = async route => {
    const request = route.request();
    if (request.method().toUpperCase() !== "POST") return route.fallback();
    if (stopped || !armed || seen) { fail(ambiguous()); return route.abort("blockedbyclient"); }
    seen = true; // Reserve before any asynchronous route operation, including a rejected first POST.
    if (!matchesBody(request.postData(), expected)) { fail(mismatch()); return route.abort("blockedbyclient"); }
    admitted = true;
    // This final continuation intentionally owns this exact POST, rather than delegating to a
    // later handler that could rewrite the validated body. Other requests/handlers are retained.
    return route.continue();
  };
  const handler = route => {
    const work = handle(route).catch(async () => {
      fail(mismatch());
      try { await route.abort("blockedbyclient"); } catch { /* Already failed; never report success. */ }
    });
    active.add(work);
    void work.then(() => active.delete(work));
    return work;
  };
  const remove = () => (removal ??= page.unroute(matches, handler));
  const ready = page.route(matches, handler).then(() => stopped ? remove() : undefined);
  void ready.catch(() => undefined);
  return {
    ready,
    arm() { if (stopped) throw ambiguous(); armed = true; },
    admitted: () => admitted,
    async stop() {
      stopped = true;
      let timer;
      try {
        await Promise.race([(async () => { await ready; await remove(); await Promise.allSettled([...active]); })(),
          new Promise((_, reject) => { timer = setTimeout(() => reject(mismatch()), 5000); })]);
      } finally { clearTimeout(timer); }
    },
  };
}
