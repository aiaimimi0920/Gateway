import net from "node:net";
import crypto from "node:crypto";
import { DEFAULT_LOCAL_WS_PORT } from "./settings.mjs";

export function createWebSocketTextFrame(text) {
  const payload = Buffer.from(text, "utf8");
  const header =
    payload.length < 126
      ? Buffer.from([0x81, payload.length])
      : payload.length < 65536
        ? Buffer.from([0x81, 126, payload.length >> 8, payload.length & 0xff])
        : (() => {
            const frame = Buffer.alloc(10);
            frame[0] = 0x81;
            frame[1] = 127;
            frame.writeBigUInt64BE(BigInt(payload.length), 2);
            return frame;
          })();
  return Buffer.concat([header, payload]);
}

export function parseWebSocketFrames(buffer) {
  const frames = [];
  let offset = 0;
  while (offset + 2 <= buffer.length) {
    const first = buffer[offset];
    const second = buffer[offset + 1];
    const opcode = first & 0x0f;
    const masked = (second & 0x80) !== 0;
    let payloadLength = second & 0x7f;
    let headerLength = 2;
    if (payloadLength === 126) {
      if (offset + 4 > buffer.length) break;
      payloadLength = buffer.readUInt16BE(offset + 2);
      headerLength = 4;
    } else if (payloadLength === 127) {
      if (offset + 10 > buffer.length) break;
      payloadLength = Number(buffer.readBigUInt64BE(offset + 2));
      headerLength = 10;
    }
    const maskLength = masked ? 4 : 0;
    const totalLength = headerLength + maskLength + payloadLength;
    if (offset + totalLength > buffer.length) break;
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

export async function createLightweightLocalWebSocketServer(port = DEFAULT_LOCAL_WS_PORT) {
  const liveSockets = new Set();
  let connectionCount = 0;

  const server = net.createServer((socket) => {
    connectionCount += 1;
    liveSockets.add(socket);
    let handshakeDone = false;
    let pending = Buffer.alloc(0);

    socket.on("data", (chunk) => {
      try {
        pending = Buffer.concat([pending, chunk]);
        if (!handshakeDone) {
          const marker = pending.indexOf("\r\n\r\n");
          if (marker === -1) {
            return;
          }
          const head = pending.slice(0, marker).toString("utf8");
          pending = pending.slice(marker + 4);
          const lines = head.split("\r\n");
          const [, ...headerLines] = lines;
          const headers = Object.fromEntries(
            headerLines
              .map((line) => {
                const idx = line.indexOf(":");
                if (idx === -1) return null;
                return [line.slice(0, idx).trim().toLowerCase(), line.slice(idx + 1).trim()];
              })
              .filter(Boolean),
          );
          const wsKey = headers["sec-websocket-key"];
          if (!wsKey) {
            socket.destroy();
            return;
          }
          const accept = crypto
            .createHash("sha1")
            .update(`${wsKey}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`, "utf8")
            .digest("base64");
          socket.write(
            "HTTP/1.1 101 Switching Protocols\r\n" +
              "Upgrade: websocket\r\n" +
              "Connection: Upgrade\r\n" +
              `Sec-WebSocket-Accept: ${accept}\r\n` +
              "\r\n",
          );
          handshakeDone = true;
        }

        if (handshakeDone && pending.length > 0) {
          const parsed = parseWebSocketFrames(pending);
          pending = parsed.rest;
          for (const frame of parsed.frames) {
            if (frame.opcode === 0x9) {
              socket.write(Buffer.from([0x8a, 0x00]));
            } else if (frame.opcode === 0x1) {
              const text = frame.payload.toString("utf8");
              if (text.includes("\"type\":\"ping\"")) {
                socket.write(createWebSocketTextFrame(JSON.stringify({ type: "pong" })));
              }
            }
          }
        }
      } catch (_) {}
    });

    socket.on("close", () => {
      liveSockets.delete(socket);
    });
    socket.on("error", () => {
      liveSockets.delete(socket);
    });
  });

  try {
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(Number(port), "127.0.0.1", () => {
        server.off("error", reject);
        resolve();
      });
    });
  } catch (error) {
    if (error?.code === "EADDRINUSE") {
      return {
        reusedExisting: true,
        waitForConnection: async () => true,
        close: async () => {},
      };
    }
    throw error;
  }

  return {
    reusedExisting: false,
    waitForConnection: async (timeoutMs = 20_000) => {
      const startedAt = Date.now();
      while (Date.now() - startedAt < timeoutMs) {
        if (connectionCount > 0) {
          return true;
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
      return connectionCount > 0;
    },
    close: async () =>
      new Promise((resolve, reject) => {
        for (const socket of liveSockets) {
          try {
            socket.destroy();
          } catch (_) {}
        }
        server.close((error) => (error ? reject(error) : resolve()));
      }),
  };
}
