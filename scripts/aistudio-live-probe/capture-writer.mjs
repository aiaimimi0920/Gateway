export function createCaptureWriter(writeSnapshot) {
  let running = null;
  let dirty = false;
  let stopped = false;
  return {
    request() {
      if (stopped) return Promise.resolve();
      dirty = true;
      if (running) return running;
      // Coalesce callers into one drain instead of retaining a promise/snapshot
      // queue. Serialize the latest mutable capture only when a write starts.
      running = Promise.resolve().then(async () => {
        try {
          while (dirty && !stopped) {
            dirty = false;
            await writeSnapshot();
          }
        } finally {
          running = null;
        }
      });
      return running;
    },
    async stop() {
      stopped = true;
      dirty = false;
      await running;
    },
  };
}
