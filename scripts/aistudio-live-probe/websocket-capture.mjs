import net from "node:net";
import crypto from "node:crypto";
import { normalizeString, tryParseJson, previewText } from "./input-text.mjs";
import { createPendingBufferPool } from "./pending-buffer.mjs";

const MAX_HANDSHAKE_BYTES = 16 * 1024;
const MAX_FRAME_BYTES = 16 * 1024 * 1024;
const MAX_CONNECTIONS = 64;
const MAX_RECEIVED_FRAMES = 4096;
const MAX_SENT_FRAMES = 4096;
const MAX_CAPTURE_TEXT_BYTES = 4 * 1024 * 1024;

function createWebSocketTextFrame(text) {
  const payload = Buffer.from(String(text), "utf8");
  if (payload.length < 126) {
    return Buffer.concat([Buffer.from([0x81, payload.length]), payload]);
  }
  if (payload.length < 65536) {
    const header = Buffer.alloc(4);
    header[0] = 0x81;
    header[1] = 126;
    header.writeUInt16BE(payload.length, 2);
    return Buffer.concat([header, payload]);
  }
  const header = Buffer.alloc(10);
  header[0] = 0x81;
  header[1] = 127;
  header.writeBigUInt64BE(BigInt(payload.length), 2);
  return Buffer.concat([header, payload]);
}

function parseWebSocketFrames(buffer, remainingFrames) {
  const frames = [];
  let offset = 0;
  while (offset + 2 <= buffer.length) {
    const first = buffer[offset];
    const second = buffer[offset + 1];
    const opcode = first & 0x0f;
    const masked = Boolean(second & 0x80);
    let payloadLength = second & 0x7f;
    let headerLength = 2;

    if (payloadLength === 126) {
      if (offset + 4 > buffer.length) break;
      payloadLength = buffer.readUInt16BE(offset + 2);
      headerLength = 4;
    } else if (payloadLength === 127) {
      if (offset + 10 > buffer.length) break;
      const bigLength = buffer.readBigUInt64BE(offset + 2);
      if (bigLength > BigInt(MAX_FRAME_BYTES)) {
        throw new Error("WebSocket frame exceeds 16 MiB.");
      }
      payloadLength = Number(bigLength);
      headerLength = 10;
    }

    const maskLength = masked ? 4 : 0;
    const totalLength = headerLength + maskLength + payloadLength;
    if (offset + totalLength > buffer.length) break;
    if (frames.length >= remainingFrames) {
      throw new Error("WebSocket received frame count exceeds 4096.");
    }

    let payload = buffer.slice(offset + headerLength + maskLength, offset + totalLength);
    if (masked) {
      const mask = buffer.slice(offset + headerLength, offset + headerLength + 4);
      const unmasked = Buffer.alloc(payload.length);
      for (let i = 0; i < payload.length; i += 1) {
        unmasked[i] = payload[i] ^ mask[i % 4];
      }
      payload = unmasked;
    }

    frames.push({ opcode, payload, totalLength });
    offset += totalLength;
  }

  return {
    frames,
    rest: buffer.slice(offset),
  };
}

async function createLocalWebSocketCaptureServer(port, capture, persistCapture) {
  if (!Number.isFinite(Number(port)) || Number(port) <= 0) {
    return null;
  }

  capture.localWebSocket = {
    host: "127.0.0.1",
    port: Number(port),
    connections: [],
  };
  const liveSockets = new Set();
  const socketClosures = new Set();
  const persistenceTasks = new Set();
  let closing = false;
  let closePromise = null;
  const pendingPool = createPendingBufferPool();
  let receivedFrameCount = 0;
  let sentFrameCount = 0;
  let captureTextBytes = 0;
  const appendCapturedText = (entries, text) => {
    const bytes = Buffer.byteLength(text, "utf8");
    if (bytes > MAX_CAPTURE_TEXT_BYTES - captureTextBytes) {
      throw new Error("WebSocket capture text exceeds 4 MiB.");
    }
    captureTextBytes += bytes;
    entries.push(text);
  };

  const server = net.createServer((socket) => {
    // Closed connections remain in the report, so admission is a lifetime cap.
    if (closing || capture.localWebSocket.connections.length >= MAX_CONNECTIONS) {
      socket.destroy();
      return;
    }
    const connection = {
      openedAt: new Date().toISOString(),
      remoteAddress: socket.remoteAddress,
      remotePort: socket.remotePort,
      handshakeRequest: null,
      handshakeHeaders: null,
      framesReceived: [],
      framesSent: [],
      receivedEventTypes: [],
      sentEventTypes: [],
      dispatchKeys: [],
      errors: [],
      closedAt: null,
    };
    capture.localWebSocket.connections.push(connection);
    const liveEntry = { socket, connection };
    liveSockets.add(liveEntry);

    let handshakeDone = false;
    const pending = pendingPool.createBuffer();
    const recordError = (message) => {
      if (connection.errors.length < 8) connection.errors.push(message);
    };
    const handshakeDeadline = setTimeout(() => {
      clearTimeout(handshakeDeadline);
      if (handshakeDone || socket.destroyed) return;
      recordError("WebSocket handshake did not finish within 10 seconds.");
      pending.release();
      socket.destroy();
    }, 10000);
    handshakeDeadline.unref();
    const persistConnection = () => {
      const task = Promise.resolve().then(persistCapture).catch(() => {
        recordError("Local WebSocket capture persistence failed.");
        socket.destroy();
      }).finally(() => persistenceTasks.delete(task));
      persistenceTasks.add(task);
      return task;
    };

    socket.on("data", (chunk) => {
      if (closing || socket.destroyed) return;
      try {
        pending.append(chunk);
        if (!handshakeDone) {
          const marker = pending.bytes().indexOf("\r\n\r\n");
          if (marker === -1 ? pending.length > MAX_HANDSHAKE_BYTES : marker + 4 > MAX_HANDSHAKE_BYTES) {
            throw new Error("WebSocket handshake exceeds 16 KiB.");
          }
          if (marker === -1) {
            return;
          }
          const head = pending.bytes().subarray(0, marker).toString("utf8");
          pending.consume(marker + 4);
          const lines = head.split("\r\n");
          const [requestLine, ...headerLines] = lines;
          const headers = Object.fromEntries(
            headerLines
              .map((line) => {
                const idx = line.indexOf(":");
                if (idx === -1) return null;
                return [line.slice(0, idx).trim().toLowerCase(), line.slice(idx + 1).trim()];
              })
              .filter(Boolean),
          );
          connection.handshakeRequest = requestLine;
          connection.handshakeHeaders = headers;
          const wsKey = headers["sec-websocket-key"];
          if (!wsKey) {
            throw new Error("Missing Sec-WebSocket-Key in local probe handshake.");
          }
          const accept = crypto
            .createHash("sha1")
            .update(`${wsKey}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`, "utf8")
            .digest("base64");
          const response =
            "HTTP/1.1 101 Switching Protocols\r\n" +
            "Upgrade: websocket\r\n" +
            "Connection: Upgrade\r\n" +
            `Sec-WebSocket-Accept: ${accept}\r\n` +
            "\r\n";
          socket.write(response);
          handshakeDone = true;
          clearTimeout(handshakeDeadline);
          connection.framesSent.push("[handshake-101]");
        }

        if (handshakeDone && pending.length > 0) {
          const parsed = parseWebSocketFrames(pending.bytes(), MAX_RECEIVED_FRAMES - receivedFrameCount);
          const consumed = pending.length - parsed.rest.length;
          for (const frame of parsed.frames) {
            receivedFrameCount += 1;
            if (frame.opcode === 0x1) {
              const text = frame.payload.toString("utf8");
              const parsedFrame = tryParseJson(text);
              if (typeof parsedFrame?.event_type === "string") {
                appendCapturedText(connection.receivedEventTypes, previewText(parsedFrame.event_type, 256));
              }
              appendCapturedText(connection.framesReceived, previewText(text, 12000));
            } else if (frame.opcode === 0x2) {
              appendCapturedText(connection.framesReceived, `[binary ${frame.payload.length} bytes]`);
            } else if (frame.opcode === 0x8) {
              appendCapturedText(connection.framesReceived, "[close]");
            } else if (frame.opcode === 0x9) {
              appendCapturedText(connection.framesReceived, "[ping]");
              socket.write(Buffer.from([0x8a, 0x00]));
              connection.framesSent.push("[pong]");
            } else {
              appendCapturedText(connection.framesReceived,
                `[opcode ${frame.opcode}] ${previewText(frame.payload.toString("utf8"), 2048)}`,
              );
            }
          }
          // Drop parser views before releasing capacity or awaiting persistence.
          parsed.frames.length = 0;
          parsed.rest = null;
          pending.consume(consumed);
        }
        // Finish parsing before persistence can yield to another socket event.
        void persistConnection();
      } catch (error) {
        pending.release();
        recordError(String(error));
        socket.destroy();
        void persistConnection();
      }
    });

    socket.on("error", (error) => {
      recordError(String(error));
      void persistConnection();
    });

    socket.on("close", () => {
      clearTimeout(handshakeDeadline);
      liveSockets.delete(liveEntry);
      pending.release();
      connection.closedAt = new Date().toISOString();
      void persistConnection();
    });
    const closed = new Promise((resolve) => socket.once("close", resolve));
    socketClosures.add(closed);
    void closed.then(() => socketClosures.delete(closed));
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(Number(port), "127.0.0.1", () => {
      server.off("error", reject);
      resolve();
    });
  });

  return {
    sendMessages: async (messages, options = {}) => {
      if (closing) throw new Error("Local WebSocket capture server is closed.");
      const normalized = Array.isArray(messages) ? messages : [];
      const dispatchKey = normalizeString(options?.dispatchKey);
      if (normalized.length > MAX_SENT_FRAMES) {
        throw new Error("WebSocket outbound frame count exceeds 4096.");
      }
      if (dispatchKey && dispatchKey.length > 1024) {
        throw new Error("WebSocket dispatch key exceeds 1024 characters.");
      }
      const onlyUndispatched = options?.onlyUndispatched === true;
      let sentConnectionCount = 0;
      for (const entry of liveSockets) {
        if (entry.socket.destroyed) {
          continue;
        }
        if (
          onlyUndispatched &&
          dispatchKey &&
          Array.isArray(entry.connection.dispatchKeys) &&
          entry.connection.dispatchKeys.includes(dispatchKey)
        ) {
          continue;
        }
        let sentToThisConnection = false;
        for (const message of normalized) {
          if (sentFrameCount >= MAX_SENT_FRAMES) {
            throw new Error("WebSocket outbound frame count exceeds 4096.");
          }
          const text = typeof message === "string" ? message : JSON.stringify(message ?? null);
          const payloadBytes = Buffer.byteLength(text, "utf8");
          if (payloadBytes > MAX_FRAME_BYTES) {
            throw new Error("WebSocket outbound frame exceeds 16 MiB.");
          }
          if (entry.socket.writableLength + payloadBytes + 10 > MAX_FRAME_BYTES + 10) {
            entry.socket.destroy();
            throw new Error("WebSocket outbound queue exceeds 16 MiB.");
          }
          const parsedMessage = tryParseJson(text);
          if (typeof parsedMessage?.event_type === "string") {
            appendCapturedText(entry.connection.sentEventTypes, previewText(parsedMessage.event_type, 256));
          }
          appendCapturedText(entry.connection.framesSent, previewText(text, 12000));
          sentFrameCount += 1;
          entry.socket.write(createWebSocketTextFrame(text));
          sentToThisConnection = true;
        }
        if (sentToThisConnection) {
          sentConnectionCount += 1;
          if (dispatchKey) {
            appendCapturedText(entry.connection.dispatchKeys, dispatchKey);
          }
        }
      }
      const persistence = Promise.resolve().then(persistCapture);
      persistenceTasks.add(persistence);
      try {
        await persistence;
      } finally {
        persistenceTasks.delete(persistence);
      }
      return {
        sentConnectionCount,
      };
    },
    close: () => {
      if (closePromise) return closePromise;
      closing = true;
      const closedSockets = [...socketClosures];
      closePromise = (async () => {
        const stopped = new Promise((resolve, reject) => {
          server.close((error) => (error ? reject(error) : resolve()));
        });
        for (const entry of liveSockets) {
          entry.socket.destroy();
        }
        await Promise.all([stopped, ...closedSockets]);
        // Socket close handlers enqueue their final timestamps before resolving.
        await Promise.all([...persistenceTasks]);
      })();
      return closePromise;
    },
  };
}

export { createLocalWebSocketCaptureServer };
