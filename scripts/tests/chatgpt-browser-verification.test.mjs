import assert from "node:assert/strict";
import test from "node:test";
import { loginBrowserSession } from "../chatgpt-web-session/browser-login.mjs";

function fixture() {
  const calls = [];
  let reads = 0;
  let permit;
  const signal = new Promise((resolve) => { permit = resolve; });
  return { calls, permit, get reads() { return reads; }, options: {
    createSession: async () => ({
      waitForWindowClosed: async () => calls.push("login_closed"),
      startCapture: async () => { calls.push("capture"); return 54321; },
      isAlive: () => true,
      prepareVerification: async () => calls.push("retain_profile"),
      openVerification: async () => calls.push("human_verification_closed"),
      close: async () => calls.push("cleanup"),
    }),
    readSession: async ({ operation }) => {
      if (operation === "list") return { tabs: [{ id: "owned" }] };
      if (++reads === 1) throw new Error("session_verification_required");
      return { material: { authToken: "synthetic" }, accountName: "synthetic@example.test" };
    },
    interaction: {
      setPhase: async (phase) => calls.push(phase),
      waitForVerification: async (_deadline, abortSignal) => {
        await Promise.race([signal, new Promise((_, reject) => {
          abortSignal.addEventListener("abort", () => reject(new Error("verification_cancelled")), { once: true });
        })]);
      },
    },
  } };
}

test("challenge suspends reads until explicit user action, then reuses the owned profile once", async () => {
  const f = fixture();
  const pending = loginBrowserSession({ ...f.options, workTimeoutMs: 1000, pollMs: 1 });
  const waitUntilReady = async () => {
    for (let attempt = 0; attempt < 100; attempt++) {
      if (f.calls.includes("verification_required")) return;
      await new Promise((resolve) => setTimeout(resolve, 1));
    }
    assert.fail("verification state was not exposed");
  };
  await Promise.race([waitUntilReady(), pending]);
  assert.equal(f.reads, 1);
  assert.ok(f.calls.includes("retain_profile"));
  assert.equal(f.calls.includes("human_verification_closed"), false);
  f.permit();
  const result = await pending;
  assert.equal(result.accountName, "synthetic@example.test");
  assert.equal(f.calls.filter((call) => call === "capture").length, 2);
  assert.equal(f.calls.at(-1), "cleanup");
});

test("a repeated challenge or ordinary 403 never opens another verification window", async () => {
  for (const code of ["session_verification_required", "session_access_forbidden"]) {
    const f = fixture(); f.permit();
    await assert.rejects(loginBrowserSession({ ...f.options, pollMs: 1,
      readSession: async ({ operation }) => {
        if (operation === "list") return { tabs: [{ id: "owned" }] };
        throw new Error(code);
      },
    }), (error) => error.message === code);
    assert.equal(f.calls.filter((call) => call === "human_verification_closed").length, code === "session_verification_required" ? 1 : 0);
    assert.equal(f.calls.at(-1), "cleanup");
  }
});

test("verification wait obeys the transaction deadline and cannot launch a late browser", async () => {
  const f = fixture();
  await assert.rejects(loginBrowserSession({ ...f.options, workTimeoutMs: 15, pollMs: 1 }), /browser_work_timeout/);
  f.permit(); await new Promise((resolve) => setTimeout(resolve, 5));
  assert.equal(f.calls.includes("human_verification_closed"), false);
  assert.equal(f.calls.at(-1), "cleanup");
});

test("failed capture shutdown does not open a conflicting window or misreport failed sign-in", async () => {
  const f = fixture();
  await assert.rejects(loginBrowserSession({ ...f.options, createSession: async () => ({
    ...await f.options.createSession(), prepareVerification: async () => { throw new Error("private shutdown detail"); },
  }) }), (error) => error.message === "verification_prepare_failed");
  assert.equal(f.calls.includes("human_verification_closed"), false);
  assert.equal(f.calls.at(-1), "cleanup");
});

test("timeouts during shutdown, human verification, or recapture never resume late work", async () => {
  for (const stage of ["prepare", "human", "recapture"]) {
    const f = fixture(); f.permit();
    let release;
    const gate = new Promise((resolve) => { release = resolve; });
    let captures = 0;
    await assert.rejects(loginBrowserSession({ ...f.options, workTimeoutMs: 15,
      createSession: async () => ({ ...await f.options.createSession(),
        prepareVerification: async () => { if (stage === "prepare") await gate; },
        openVerification: async () => { f.calls.push("human_open"); if (stage === "human") await gate; },
        startCapture: async () => { if (++captures === 2 && stage === "recapture") await gate; return 54321; },
      }),
    }), /browser_work_timeout/);
    assert.equal(f.calls.at(-1), "cleanup");
    release(); await new Promise((resolve) => setTimeout(resolve, 5));
    assert.equal(f.reads, 1, `${stage}: no late authenticated read`);
    assert.equal(captures, stage === "recapture" ? 2 : 1);
    assert.equal(f.calls.includes("human_open"), stage !== "prepare");
  }
});
