import assert from "node:assert/strict";
import test from "node:test";
import { executionFixture } from "./gemini-canvas-program-handle.execution-fixtures.mjs";
import { requestFixture, responseFixture, deferred, turn, streamUrl, pairText } from "./gemini-canvas-program-handle.network-fixtures.mjs";

const events = ["request", "response", "websocket"];
const count = (f, name) => f.calls.filter(([kind]) => kind === name).length;
const outcome = async (f, failAt) => {
  if (failAt) await assert.rejects(f.run(), (error) => error === f.failure);
  else assert.equal(await f.run(), undefined);
};

for (const failAt of [undefined, "snapshot after"]) {
  const ending = failAt ? "rejection" : "success";
  for (const adopt of [false, true]) {
    test("standalone final cleanup releases owned listeners after " + ending + (adopt ? " with adoption" : ""), async (t) => {
      const f = await executionFixture(t, { failAt, follow: adopt ? ["popup"] : [] });
      const inherited = () => {};
      for (const page of [f.initial, f.popup]) for (const event of events) page.on(event, inherited);
      let listenersAtClose;
      const context = f.initial.context(), close = context.close;
      context.close = async () => {
        listenersAtClose = [f.initial, f.popup].map((page) => events.map((event) => page.listeners(event)));
        await close();
      };
      await outcome(f, failAt);
      assert.deepEqual(listenersAtClose, [[...events.map(() => [inherited])], [...events.map(() => [inherited])]]);
      for (const page of [f.initial, f.popup]) for (const event of events) assert.deepEqual(page.listeners(event), [inherited]);
      assert.equal(count(f, "capture stop"), 1);
      assert.equal(count(f, "context close"), 1);
      assert.deepEqual(f.calls.slice(-2).map(([name]) => name), ["capture stop", "context close"]);
      assert.equal(f.captures.length, 1);
      assert.equal(f.initial.context().listenerCount("request"), 0);
    });
  }

  test("standalone final cleanup preserves " + ending + " when capture detach throws", async (t) => {
    const f = await executionFixture(t, { failAt });
    let attempts = 0;
    const cleanupError = new Error("fixture detach failure");
    const context = f.initial.context(), off = context.off;
    context.off = (event, listener) => {
      if (event === "request") { attempts += 1; throw cleanupError; }
      return off.call(context, event, listener);
    };
    await outcome(f, failAt);
    assert.equal(attempts, 1);
    assert.equal(count(f, "capture stop"), 1);
    assert.equal(count(f, "context close"), 1);
    assert.deepEqual(f.calls.slice(-2).map(([name]) => name), ["capture stop", "context close"]);
    assert.equal(f.printed.length, failAt ? 0 : 1);
  });

  test("standalone final cleanup ignores response text completing after " + ending, async (t) => {
    const text = deferred();
    let pending;
    const f = await executionFixture(t, {
      failAt,
      snapshot(label, _index, page) {
        if (label === "share-before") {
          const [response] = page.context().listeners("response");
          pending = response(responseFixture({ request: requestFixture({ url: streamUrl }), text: () => text.promise }));
        }
      },
    });
    await outcome(f, failAt);
    assert.ok(pending);
    const finished = structuredClone(f.captures[0].state);
    text.resolve(pairText);
    await pending;
    assert.deepEqual(f.captures[0].state, finished);
    assert.equal(count(f, "context close"), 1);
  });

  test("standalone final cleanup ignores Cookie completion after " + ending, async (t) => {
    const cookies = deferred();
    let cookieReads = 0;
    const f = await executionFixture(t, {
      failAt,
      snapshot(label, _index, page) {
        if (label === "share-before") page.context().emit("request", requestFixture({ url: streamUrl }));
      },
    });
    f.initial.context().cookies = () => { cookieReads += 1; return cookies.promise; };
    await outcome(f, failAt);
    assert.equal(cookieReads, 1);
    const finished = structuredClone(f.captures[0].state);
    cookies.resolve([{ name: "fixture", value: "late" }]);
    await turn();
    assert.deepEqual(f.captures[0].state, finished);
    assert.equal(count(f, "context close"), 1);
  });
}

for (const failAt of ["launch", "pages", "capture"]) {
  test("standalone final cleanup skips unacquired capture after " + failAt, async (t) => {
    const f = await executionFixture(t, { failAt });
    await outcome(f, failAt);
    assert.equal(f.captures.length, 0);
    assert.equal(count(f, "capture stop"), 0);
    assert.equal(count(f, "context close"), failAt === "launch" ? 0 : 1);
  });
}
