// Explicit --import test loader only. This is not the native byte-admission proof.
export function createProgramResponseCapture(page, options) {
  let stopped = false, task, count = 0;
  const pump = async () => {
    if (stopped) return;
    const raw = await page.waitForResponse();
    if (stopped) return;
    const path = count++ === 0 ? "/api/generate/v2-web/" : "/api/feed/v3";
    options.onResponse({
      url: () => `https://suno.fixture.invalid${path}`, status: raw.status,
      headers: () => ({ "content-type": "application/json" }),
      request: () => ({ method: () => "POST" }), text: raw.text,
    });
    task = setImmediate(pump);
  };
  task = setImmediate(pump);
  return { ready: Promise.resolve(), async stop() { stopped = true; clearImmediate(task); } };
}
