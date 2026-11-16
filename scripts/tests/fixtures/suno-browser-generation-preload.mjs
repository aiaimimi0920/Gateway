// Algorithm fixture only; native decoded-byte capture has separate real-CDP tests.
import "./suno-worker-lifecycle-preload.mjs";
import { chromium } from "playwright-core";
const mode = process.env.SUNO_GENERATION_FIXTURE;
const clip = (id, status = "submitted") => ({ id, status,
  ...(status === "complete" && { audio_url: `https://asset.invalid/${id}` }) });
const original = chromium.connectOverCDP.bind(chromium);
chromium.connectOverCDP = async (...args) => {
  const browser = await original(...args);
  const page = browser.contexts()[0].pages()[0];
  let call = 0;
  const now = Date.now.bind(Date);
  let elapsed = 0;
  Date.now = () => now() + elapsed;
  page.waitForResponse = async () => {
    let clips;
    if (call++ === 0) {
      clips = mode === "failed-submit" ? [clip("A", "failed")]
        : mode === "malformed-submit" ? [clip("A"), { status: "submitted" }]
        : [clip("A"), clip("B")];
    } else if (mode === "partial") {
      clips = call === 2 ? [clip("A", "complete")] : [clip("B", "complete"), clip("A", "complete")];
    } else if (mode === "duplicate") {
      clips = [clip("A", "complete"), clip("A", "complete")];
    } else if (mode === "failed-feed") {
      clips = [clip("A", "complete"), clip("B", "error")];
    } else if (mode === "foreign") {
      clips = [clip("foreign", "complete")]; elapsed += 20000;
    } else if (mode === "disjoint") {
      clips = [clip(call === 2 ? "A" : "B", "complete")];
      if (call > 2) elapsed += 20000;
    } else if (mode === "timeout") {
      clips = []; elapsed += 20000;
    } else clips = [clip("B", "complete"), clip("A", "complete")];
    return { status: () => 200, text: async () => JSON.stringify({ clips }) };
  };
  return browser;
};
