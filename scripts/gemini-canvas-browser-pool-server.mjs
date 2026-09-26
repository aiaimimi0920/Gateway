import { mkdir } from "node:fs/promises";
import { WebSocketServer } from "ws";
import { createServer } from "node:http";
import { createServer as createHttpsServer } from "node:https";
import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES } from "./gemini-canvas-browser-pool-body.mjs";
import { loadOrCreateTlsCertificate } from "./gemini-canvas-browser-pool-tls.mjs";
import { getStorageRoot } from "./gemini-canvas-browser-pool-runtime-state.mjs";

export function createBrowserPoolServerOwner({
  DEFAULT_HOST,
  DEFAULT_PORT,
  DEFAULT_TLS_PORT,
  evictIdleContexts,
  log,
  contexts,
  listConnectedClients,
  nextConnectedRequestId,
  connectedClients,
  cleanupConnectedClient,
  handleConnectedClientMessage,
  invokeGeminiCanvas,
}) {
  function sendJson(res, status, body) {
    const json = Buffer.from(JSON.stringify(body));
    res.writeHead(status, {
      "content-type": "application/json; charset=utf-8",
      "content-length": String(json.length),
      "cache-control": "no-store",
    });
    res.end(json);
  }

  function readJsonBody(req) {
    return new Promise((resolve, reject) => {
      const chunks = [];
      req.on("data", (chunk) => chunks.push(Buffer.from(chunk)));
      req.on("end", () => {
        try {
          const raw = Buffer.concat(chunks).toString("utf8");
          resolve(raw ? JSON.parse(raw) : {});
        } catch (error) {
          reject(error);
        }
      });
      req.on("error", reject);
    });
  }

  async function main() {
    const host = normalizeString(process.env.GEMINI_CANVAS_BROWSER_HOST) ?? DEFAULT_HOST;
    const port = Number(process.env.GEMINI_CANVAS_BROWSER_POOL_PORT || DEFAULT_PORT);
    const tlsPort = Number(process.env.GEMINI_CANVAS_BROWSER_POOL_TLS_PORT || DEFAULT_TLS_PORT);
    await mkdir(getStorageRoot(), { recursive: true }).catch(() => undefined);
    const wsServer = new WebSocketServer({
      noServer: true,
      maxPayload: BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES,
    });

    setInterval(() => {
      evictIdleContexts().catch((error) => log("idle eviction failed", error.message));
    }, 5 * 60 * 1000).unref();

    const requestHandler = async (req, res) => {
      try {
        if (req.method === "GET" && req.url === "/health") {
          return sendJson(res, 200, {
            ok: true,
            contexts: contexts.size,
            endpoints: {
              httpBaseUrl: `http://${host}:${port}`,
              httpsBaseUrl: `https://${host}:${tlsPort}`,
              wsEndpoint: `ws://${host}:${port}/ws`,
              wssEndpoint: `wss://${host}:${tlsPort}/ws`,
            },
            connectedClients: listConnectedClients().map((entry) => ({
              clientLabel: entry.clientLabel,
              connectedAt: entry.connectedAt,
            })),
          });
        }

        if (req.method === "POST" && req.url === "/invoke") {
          const body = await readJsonBody(req);
          const result = await invokeGeminiCanvas(body);
          return sendJson(res, result.ok ? 200 : Number(result.error?.status ?? 500), result);
        }

        if (req.method === "POST" && req.url === "/fetch") {
          const body = await readJsonBody(req);
          const result = await invokeGeminiCanvas({
            ...body,
            fetchRequest: body.fetchRequest ?? body,
          });
          return sendJson(res, result.ok ? 200 : Number(result.error?.status ?? 500), result);
        }

        return sendJson(res, 404, {
          ok: false,
          error: {
            code: "not_found",
            message: "Unknown browser pool endpoint.",
            status: 404,
          },
        });
      } catch (error) {
        return sendJson(res, 500, {
          ok: false,
          error: {
            code: "gemini_canvas_browser_pool_server_error",
            message: error instanceof Error ? error.message : String(error),
            status: 500,
          },
        });
      }
    };

    function attachUpgradeHandler(server, scheme, listenPort) {
      server.on("upgrade", (req, socket, head) => {
        try {
          const requestUrl = new URL(req.url || "/", `${scheme}://${req.headers.host || `${host}:${listenPort}`}`);
          if (requestUrl.pathname !== "/ws") {
            socket.destroy();
            return;
          }
          wsServer.handleUpgrade(req, socket, head, (ws) => {
            const connectionId = nextConnectedRequestId();
            const entry = {
              connectionId,
              ws,
              authenticated: false,
              clientLabel: null,
              connectedAt: new Date().toISOString(),
              scheme,
            };
            connectedClients.set(connectionId, entry);
            ws.on("message", (message) => handleConnectedClientMessage(connectionId, message));
            ws.on("close", () => cleanupConnectedClient(connectionId));
            ws.on("error", () => cleanupConnectedClient(connectionId));
          });
        } catch {
          socket.destroy();
        }
      });
    }

    const httpServer = createServer(requestHandler);
    attachUpgradeHandler(httpServer, "http", port);
    httpServer.listen(port, host, () => {
      log(`listening on http://${host}:${port}`);
    });

    const tlsBundle = loadOrCreateTlsCertificate(host);
    const httpsServer = createHttpsServer(
      {
        key: tlsBundle.key,
        cert: tlsBundle.cert,
      },
      requestHandler,
    );
    attachUpgradeHandler(httpsServer, "https", tlsPort);
    httpsServer.listen(tlsPort, host, () => {
      log(`listening on https://${host}:${tlsPort}`);
      log(
        `${tlsBundle.generated ? "generated" : "loaded"} TLS certificate`,
        tlsBundle.certPath,
        tlsBundle.keyPath,
      );
    });
  }

  return { main, sendJson, readJsonBody };
}
