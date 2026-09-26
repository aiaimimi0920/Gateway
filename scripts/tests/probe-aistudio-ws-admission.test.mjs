import assert from "node:assert/strict";
import net from "node:net";
import { once } from "node:events";
import test, { mock } from "node:test";
import { createLocalWebSocketCaptureServer } from "../aistudio-live-probe/websocket-capture.mjs";

async function withServer(run, persistCapture = async () => {}) {
  const reservation = net.createServer();
  reservation.listen(0, "127.0.0.1");
  await once(reservation, "listening");
  const port = reservation.address().port;
  await new Promise((resolve) => reservation.close(resolve));
  const capture = {};
  const server = await createLocalWebSocketCaptureServer(port, capture, persistCapture);
  const client = net.createConnection({ host: "127.0.0.1", port });
  client.on("error", () => {});
  const closed = new Promise((resolve) => client.once("close", resolve));
  const deadline = setTimeout(() => client.destroy(new Error("test deadline")), 3000);
  try {
    await once(client, "connect");
    await run({ client, capture, closed, server, port });
  } finally {
    clearTimeout(deadline);
    client.destroy();
    await server.close();
    await closed;
  }
}

const handshake = "GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\n"
  + "Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n";

test("AI Studio drains final socket timestamps and persistence before idempotent close", async () => {
  let release;
  const writing = new Promise((resolve) => { release = resolve; });
  await withServer(async ({ client, capture, server }) => {
    try {
      const response = once(client, "data");
      const payload = Buffer.from('{"event_type":"ready"}');
      client.write(Buffer.concat([Buffer.from(handshake), Buffer.from([0x81, payload.length]), payload]));
      await response;
      // Persistence is still blocked; coalesced upgrade/frame data must be parsed.
      assert.deepEqual(capture.localWebSocket.connections[0].receivedEventTypes, ["ready"]);
      const stopped = server.close();
      assert.equal(server.close(), stopped);
      let finished = false;
      void stopped.then(() => { finished = true; });
      await new Promise((resolve) => setTimeout(resolve, 20));
      assert.equal(finished, false);
      await assert.rejects(server.sendMessages(["late"]), /server is closed/);
      release();
      await stopped;
      assert.ok(capture.localWebSocket.connections[0].closedAt);
      const snapshot = JSON.stringify(capture);
      await new Promise((resolve) => setImmediate(resolve));
      assert.equal(JSON.stringify(capture), snapshot);
    } finally { release(); }
  }, () => writing);
});

async function upgrade(client, request = handshake) {
  const response = once(client, "data");
  client.write(request);
  assert.match((await response)[0].toString(), /101 Switching Protocols/);
}

test("AI Studio close also drains an admitted send persistence", async () => {
  let blockNext = false;
  let release;
  const writing = new Promise((resolve) => { release = resolve; });
  await withServer(async ({ client, server }) => {
    try {
      await upgrade(client);
      blockNext = true;
      const sending = server.sendMessages([]);
      await new Promise((resolve) => setImmediate(resolve));
      const stopped = server.close();
      let finished = false;
      void stopped.then(() => { finished = true; });
      await new Promise((resolve) => setTimeout(resolve, 20));
      assert.equal(finished, false);
      release();
      await Promise.all([sending, stopped]);
    } finally { release(); }
  }, () => {
    if (!blockNext) return Promise.resolve();
    blockNext = false;
    return writing;
  });
});

test("AI Studio rejects unterminated oversized handshake before retaining it", async () => {
  await withServer(async ({ client, capture, closed }) => {
    client.write(Buffer.alloc(16385, 65));
    await closed;
    const connection = capture.localWebSocket.connections[0];
    assert.equal(connection.handshakeHeaders, null);
    assert.match(connection.errors.join(" "), /handshake exceeds 16 KiB/);
  });
});

test("AI Studio accepts exactly 16 KiB handshake including terminator", async () => {
  await withServer(async ({ client, capture }) => {
    const prefix = handshake.slice(0, -2) + "X-Padding: ";
    const request = prefix + "a".repeat(16384 - Buffer.byteLength(prefix) - 4) + "\r\n\r\n";
    assert.equal(Buffer.byteLength(request), 16384);
    await upgrade(client, request);
    assert.equal(capture.localWebSocket.connections[0].errors.length, 0);
  });
});

test("AI Studio rejects a declared oversized frame before waiting for its payload", async () => {
  await withServer(async ({ client, capture, closed }) => {
    await upgrade(client);
    const header = Buffer.from([0x81, 127, 0, 0, 0, 0, 0, 0, 0, 0]);
    header.writeBigUInt64BE(BigInt(16 * 1024 * 1024 + 1), 2);
    client.write(header);
    await closed;
    const connection = capture.localWebSocket.connections[0];
    assert.deepEqual(connection.framesReceived, []);
    assert.match(connection.errors.join(" "), /frame exceeds 16 MiB/);
  });
});

test("AI Studio preserves fragmented TCP handshake and frame capture with normal dispatch", async () => {
  await withServer(async ({ client, capture, server }) => {
    const response = once(client, "data");
    client.write(handshake.slice(0, 12));
    client.write(handshake.slice(12));
    await response;
    const payload = Buffer.from('{"event_type":"ready"}');
    client.write(Buffer.from([0x81, payload.length]));
    client.write(payload);
    for (let i = 0; i < 100 && !capture.localWebSocket.connections[0].framesReceived.length; i++) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    assert.deepEqual(capture.localWebSocket.connections[0].receivedEventTypes, ["ready"]);
    const outgoing = once(client, "data");
    assert.deepEqual(await server.sendMessages([{ event_type: "proxy_request" }]), { sentConnectionCount: 1 });
    assert.match((await outgoing)[0].toString(), /proxy_request/);
  });
});

test("AI Studio persistence failure closes the socket without unhandled rejection or secret echo", async () => {
  await withServer(async ({ client, capture, closed }) => {
    client.resume();
    client.write(handshake);
    await closed;
    const errors = capture.localWebSocket.connections[0].errors;
    assert.ok(errors.length > 0 && errors.length <= 8);
    assert.ok(errors.every((error) => error === "Local WebSocket capture persistence failed."));
  }, async () => { throw new Error("synthetic-storage-secret"); });
});

test("AI Studio caps lifetime connection records even after sockets close", async () => {
  await withServer(async ({ capture, port }) => {
    for (let i = 0; i < 65; i++) {
      const socket = net.createConnection({ host: "127.0.0.1", port });
      socket.on("error", () => {});
      const closed = new Promise((resolve) => socket.once("close", resolve));
      await once(socket, "connect");
      socket.destroy();
      await closed;
    }
    assert.equal(capture.localWebSocket.connections.length, 64);
  });
});

test("AI Studio caps received frame work before allocating millions of frame records", async () => {
  await withServer(async ({ client, capture, closed }) => {
    await upgrade(client);
    client.write(Buffer.from(Array.from({ length: 4097 }, () => [0x81, 0]).flat()));
    await closed;
    const connection = capture.localWebSocket.connections[0];
    assert.ok(connection.framesReceived.length <= 4096);
    assert.match(connection.errors.join(" "), /frame count exceeds 4096/);
  });
});

test("AI Studio caps aggregate preview bytes across otherwise valid frames", async () => {
  await withServer(async ({ client, capture, closed }) => {
    await upgrade(client);
    const payload = Buffer.alloc(12000, 65);
    const header = Buffer.from([0x81, 126, 0, 0]);
    header.writeUInt16BE(payload.length, 2);
    client.write(Buffer.concat(Array.from({ length: 360 }, () => Buffer.concat([header, payload]))));
    await closed;
    const connection = capture.localWebSocket.connections[0];
    const bytes = connection.framesReceived.reduce((sum, frame) => sum + Buffer.byteLength(frame), 0);
    assert.ok(bytes <= 4 * 1024 * 1024);
    assert.match(connection.errors.join(" "), /capture text exceeds 4 MiB/);
  });
});

test("AI Studio admits exactly 4096 frames and shares their budget across connections", async () => {
  await withServer(async ({ client, capture, port }) => {
    await upgrade(client);
    client.write(Buffer.from(Array.from({ length: 4096 }, () => [0x81, 0]).flat()));
    for (let i = 0; i < 100 && capture.localWebSocket.connections[0].framesReceived.length < 4096; i++) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    assert.equal(capture.localWebSocket.connections[0].framesReceived.length, 4096);
    const second = net.createConnection({ host: "127.0.0.1", port });
    second.on("error", () => {});
    const closed = new Promise((resolve) => second.once("close", resolve));
    const deadline = setTimeout(() => second.destroy(), 2000);
    try {
      await once(second, "connect");
      await upgrade(second);
      second.write(Buffer.from([0x81, 0]));
      await closed;
      assert.equal(capture.localWebSocket.connections[1].framesReceived.length, 0);
      assert.match(capture.localWebSocket.connections[1].errors.join(" "), /frame count exceeds 4096/);
    } finally {
      clearTimeout(deadline);
      second.destroy();
      await closed;
    }
  });
});

test("AI Studio rejects oversized outbound messages before socket writes or diagnostic retention", async () => {
  await withServer(async ({ client, capture, server }) => {
    await upgrade(client);
    await assert.rejects(server.sendMessages(["x".repeat(16 * 1024 * 1024 + 1)]), /outbound frame exceeds 16 MiB/);
    assert.deepEqual(capture.localWebSocket.connections[0].framesSent, ["[handshake-101]"]);
  });
});

test("AI Studio bounds outbound batch size and dispatch-key retention", async () => {
  await withServer(async ({ client, capture, server }) => {
    await upgrade(client);
    await assert.rejects(server.sendMessages(Array(4097).fill("")), /outbound frame count exceeds 4096/);
    await assert.rejects(server.sendMessages(["ok"], { dispatchKey: "x".repeat(1025) }), /dispatch key exceeds 1024/);
    assert.deepEqual(capture.localWebSocket.connections[0].dispatchKeys, []);
    assert.deepEqual(capture.localWebSocket.connections[0].framesSent, ["[handshake-101]"]);
  });
});

test("AI Studio outbound frame budget persists across dispatch calls", async () => {
  await withServer(async ({ client, capture, server }) => {
    await upgrade(client);
    client.resume();
    assert.deepEqual(await server.sendMessages(Array(4096).fill("")), { sentConnectionCount: 1 });
    await assert.rejects(server.sendMessages([""]), /outbound frame count exceeds 4096/);
    assert.equal(capture.localWebSocket.connections[0].framesSent.length, 4097);
  });
});

test("AI Studio stops queue growth when a loopback client does not read", async () => {
  await withServer(async ({ client, server }) => {
    await upgrade(client);
    client.pause();
    await assert.rejects(server.sendMessages(Array(8).fill("x".repeat(12 * 1024 * 1024))),
      /outbound queue exceeds 16 MiB/);
  });
});

test("AI Studio pending compaction preserves completed payloads and a partial next frame", async () => {
  await withServer(async ({ client, capture }) => {
    await upgrade(client);
    client.write(Buffer.concat([Buffer.from([0x81, 5]), Buffer.from("first"),
      Buffer.from([0x81, 6]), Buffer.from("sec")]));
    for (let i = 0; i < 100 && !capture.localWebSocket.connections[0].framesReceived.length; i++) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    assert.deepEqual(capture.localWebSocket.connections[0].framesReceived, ["first"]);
    client.write("ond");
    for (let i = 0; i < 100 && capture.localWebSocket.connections[0].framesReceived.length < 2; i++) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    assert.deepEqual(capture.localWebSocket.connections[0].framesReceived, ["first", "second"]);
  });
});

test("AI Studio incomplete handshake expires once and clears its owned deadline", async () => {
  const originalSet = globalThis.setTimeout;
  const originalClear = globalThis.clearTimeout;
  const token = { unref() {} };
  let expire;
  let cleared = false;
  const timer = mock.method(globalThis, "setTimeout", (callback, ms, ...args) => {
    if (ms !== 10000) return originalSet(callback, ms, ...args);
    expire = callback;
    return token;
  });
  const cleanup = mock.method(globalThis, "clearTimeout", (id) => {
    if (id === token) cleared = true;
    else originalClear(id);
  });
  try {
    await withServer(async ({ client, capture, closed }) => {
      assert.equal(typeof expire, "function");
      client.write("GET / HTTP/1.1\r\n");
      expire();
      await closed;
      assert.equal(cleared, true);
      assert.match(capture.localWebSocket.connections[0].errors.join(" "), /handshake did not finish within 10 seconds/);
    });
    cleared = false;
    await withServer(async ({ client, capture }) => {
      await upgrade(client);
      assert.equal(cleared, true);
      expire();
      assert.equal(client.destroyed, false);
      assert.deepEqual(capture.localWebSocket.connections[0].errors, []);
    });
    cleared = false;
    await withServer(async ({ client, closed }) => {
      client.destroy();
      await closed;
    });
    assert.equal(cleared, true);
  } finally {
    timer.mock.restore();
    cleanup.mock.restore();
  }
});
