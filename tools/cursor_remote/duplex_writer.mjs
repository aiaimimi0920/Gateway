// One bounded queue for context/KV and asynchronously delivered caller tool results.
import { once } from "node:events";
import { ProbeError } from "./transport.mjs";

export function duplexWriter(stream, signal, fail) {
  let tail = Promise.resolve(), count = 0, bytes = 0, closed = false;
  const lifetime = new AbortController();
  const stopped = AbortSignal.any([signal, lifetime.signal]);
  return {
    send(reply) {
      if (closed || signal.aborted || stream.destroyed || !Buffer.isBuffer(reply) ||
          ++count > 128 || (bytes += reply.length) > 320 * 1024) {
        throw new ProbeError("duplex_write_rejected");
      }
      const write = tail.then(async () => {
        if (closed || signal.aborted || stream.destroyed) throw new ProbeError("duplex_write_closed");
        if (!stream.write(reply)) await once(stream, "drain", { signal: stopped });
      });
      // Observe rejection immediately even when a data handler does not await a reply.
      tail = write.catch((error) => { closed = true; fail(error); });
      return write;
    },
    close() { closed = true; lifetime.abort(); },
  };
}
