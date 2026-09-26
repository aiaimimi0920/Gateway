export const MAX_PENDING_BYTES = 16 * 1024 * 1024 + 16 * 1024 + 14;
const MAX_POOL_BYTES = 32 * 1024 * 1024;

export function createPendingBufferPool() {
  let allocatedBytes = 0;
  return {
    get allocatedBytes() { return allocatedBytes; },
    createBuffer() {
      let storage = null;
      let length = 0;
      const release = () => {
        allocatedBytes -= storage?.length ?? 0;
        storage = null;
        length = 0;
      };
      return {
        get length() { return length; },
        bytes() { return storage?.subarray(0, length) ?? Buffer.alloc(0); },
        append(chunk) {
          if (chunk.length > MAX_PENDING_BYTES - length) {
            throw new Error("WebSocket pending input exceeds the capture buffer limit.");
          }
          const required = length + chunk.length;
          if (required > (storage?.length ?? 0)) {
            const capacity = Math.min(MAX_PENDING_BYTES,
              Math.max(16384, required, (storage?.length ?? 0) * 2));
            // Charge both old and new capacity while growing; other sockets
            // cannot reserve memory that is still owned by the current buffer.
            if (capacity > MAX_POOL_BYTES - allocatedBytes) {
              throw new Error("WebSocket aggregate pending capacity exceeds 32 MiB.");
            }
            allocatedBytes += capacity;
            let next;
            try {
              next = Buffer.allocUnsafe(capacity);
              storage?.copy(next, 0, 0, length);
            } catch (error) {
              allocatedBytes -= capacity;
              throw error;
            }
            allocatedBytes -= storage?.length ?? 0;
            storage = next;
          }
          if (chunk.length) chunk.copy(storage, length);
          length = required;
        },
        consume(count) {
          if (!Number.isInteger(count) || count < 0 || count > length) {
            throw new Error("Invalid WebSocket pending consume count.");
          }
          if (count === length) {
            release();
          } else if (count) {
            storage.copyWithin(0, count, length);
            length -= count;
          }
        },
        release,
      };
    },
  };
}
