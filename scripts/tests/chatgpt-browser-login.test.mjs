import assert from "node:assert/strict";
import test from "node:test";
import { loginBrowserSession } from "../chatgpt-web-session/browser-login.mjs";

function fixture({ closed = false, loggedIn = true, loginTimeout = false } = {}) {
  const calls = [];
  const token = `e30.${Buffer.from(JSON.stringify({ exp: 4070908800 })).toString("base64url")}.sig`;
  const createSession = async () => ({
    waitForWindowClosed: async () => {
      calls.push("human-window-closed");
      if (loginTimeout) throw new Error("browser_timeout");
    },
    startCapture: async () => { calls.push("start-debugging"); return 54321; },
    isAlive: () => !closed,
    close: async () => { calls.push("close"); },
  });
  const readSession = async (input) => {
    assert.equal(input.port, 54321);
    calls.push(input.operation);
    if (input.operation === "list") return { tabs: [{ id: "owned-tab" }] };
    assert.equal(input.targetId, "owned-tab");
    if (!loggedIn) throw new Error("login_required");
    return { accountName: "fixture@example.test", material: { authToken: token, deviceId: "device" } };
  };
  return { calls, createSession, readSession };
}

test("native login finishes before debugging starts and captures only its owned ChatGPT page", async () => {
  const f = fixture();
  const result = await loginBrowserSession(f);
  assert.equal(result.accountName, "fixture@example.test");
  assert.equal(result.material.deviceId, "device");
  assert.deepEqual(f.calls, ["human-window-closed", "start-debugging", "list", "prepare", "close"]);
});
test("a closed capture browser stops the read and releases its owned profile", async () => {
  const f = fixture({ closed: true });
  await assert.rejects(loginBrowserSession(f), /browser_closed/);
  assert.equal(f.calls.at(-1), "close");
});
test("unauthenticated sessions never save material and release the owned browser", async () => {
  const f = fixture({ loggedIn: false });
  await assert.rejects(loginBrowserSession({ ...f, captureMs: 1, pollMs: 1 }), /login_required/);
  assert.equal(f.calls.at(-1), "close");
});

test("human login timeout never enables debugging", async () => {
  const f = fixture({ loginTimeout: true });
  await assert.rejects(loginBrowserSession(f), /browser_timeout/);
  assert.deepEqual(f.calls, ["human-window-closed", "close"]);
});

test("ambiguous pages are not selected implicitly", async () => {
  const f = fixture();
  await assert.rejects(loginBrowserSession({ ...f, captureMs: 1, pollMs: 1,
    readSession: async () => ({ tabs: [{ id: "one" }, { id: "two" }] }),
  }), /tab_unavailable/);
  assert.equal(f.calls.at(-1), "close");
});

test("the total work budget releases the owned browser even when a session read stalls", async () => {
  const f = fixture();
  await assert.rejects(loginBrowserSession({ ...f, workTimeoutMs: 5,
    readSession: async () => new Promise(() => {}),
  }), /browser_work_timeout/);
  assert.equal(f.calls.at(-1), "close");
});

test("debugger readiness precedes document/auth readiness and transient reads are retried", async () => {
  const f = fixture();
  const failures = ["session_page_unavailable", "session_request_failed", "login_required"];
  let attempts = 0;
  const result = await loginBrowserSession({ ...f, captureMs: 1000, pollMs: 1,
    readSession: async (input) => {
      if (input.operation === "prepare" && attempts < failures.length) throw new Error(failures[attempts++]);
      return await f.readSession(input);
    },
  });
  assert.equal(attempts, failures.length);
  assert.equal(result.accountName, "fixture@example.test");
  assert.equal(f.calls.at(-1), "close");
});

test("an explicit verification or forbidden response stops without polling or masking it as a timeout", async () => {
  for (const code of ["session_verification_required", "session_access_forbidden"]) {
    const f = fixture();
    let reads = 0;
    await assert.rejects(loginBrowserSession({ ...f, workTimeoutMs: 30, captureMs: 1000, pollMs: 1,
      readSession: async (input) => {
        if (input.operation === "prepare") { reads++; throw new Error(code); }
        return f.readSession(input);
      },
    }), (error) => error.message === code);
    assert.equal(reads, 1);
    assert.equal(f.calls.at(-1), "close");
  }
});

test("a transient CDP read timeout retries instead of discarding the logged-in browser", async () => {
  const f = fixture();
  let reads = 0;
  const result = await loginBrowserSession({ ...f, captureMs: 1000, pollMs: 1,
    readSession: async (input) => {
      if (input.operation === "prepare" && reads++ === 0) throw new Error("session_read_timeout");
      return f.readSession(input);
    } });
  assert.ok(result.material.authToken);
  assert.equal(reads, 2);
});

test("cleanup failure cannot replace successfully captured material with a login timeout", async () => {
  const f = fixture();
  const result = await loginBrowserSession({ ...f, createSession: async () => ({
    ...await f.createSession(), close: async () => { throw new Error("browser_timeout"); },
  }) });
  assert.ok(result.material.authToken);
  assert.equal(result.cleanupPending, true);
});

test("cleanup failure preserves the original read failure and marks supervisor cleanup pending", async () => {
  const f = fixture();
  await assert.rejects(loginBrowserSession({ ...f, createSession: async () => ({
    ...await f.createSession(), close: async () => { throw new Error("browser_timeout"); },
  }), readSession: async () => { throw new Error("invalid_debugger"); } }),
  (error) => error.message === "invalid_debugger" && error.cleanupPending === true);
});
