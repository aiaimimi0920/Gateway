function captureError(code, message, status = 502) {
  return Object.assign(new Error(message), { code, status });
}

function callbackFailure(error) {
  if (error?.code === "aistudio_probe_capture_budget_exceeded") {
    return captureError(error.code, "AI Studio browser capture budget exceeded.", 413);
  }
  return captureError("aistudio_probe_capture_callback_failed", "AI Studio browser capture callback failed.");
}

export function createCaptureTasks(onStop) {
  const pending = new Set();
  let open = true;
  let failure = null;
  const close = () => {
    if (!open) return;
    open = false;
    onStop();
  };
  const fail = (error) => {
    if (failure) return;
    failure = error;
    close();
    for (const task of pending) task.controller.abort(error);
  };
  return {
    get open() { return open; },
    check() { if (failure) throw failure; },
    reportFailure(error) {
      fail(callbackFailure(error));
    },
    run(operation) {
      if (!open) return;
      if (pending.size >= 64) {
        fail(captureError("aistudio_probe_capture_overloaded", "AI Studio capture exceeds 64 pending callbacks.", 503));
        return;
      }
      const controller = new AbortController();
      const task = { controller, promise: null };
      pending.add(task);
      let onAbort;
      const cancelled = new Promise((_, reject) => {
        onAbort = () => reject(controller.signal.reason);
        controller.signal.addEventListener("abort", onAbort, { once: true });
      });
      const timer = setTimeout(() => fail(captureError(
        "aistudio_probe_capture_timeout", "AI Studio capture callback exceeded 30 seconds.", 504,
      )), 30000);
      const operationResult = Promise.resolve().then(() => {
        controller.signal.throwIfAborted();
        return operation(controller.signal);
      });
      task.promise = Promise.race([operationResult, cancelled]).catch((error) => {
        fail(callbackFailure(error));
      }).finally(() => {
        clearTimeout(timer);
        controller.signal.removeEventListener("abort", onAbort);
        pending.delete(task);
      });
    },
    async stop() {
      // Abort settles logical callbacks; non-cancellable native operations may
      // finish later and must check their signal before publishing mutations.
      close();
      await Promise.all([...pending].map((task) => task.promise));
      if (failure) throw failure;
    },
  };
}
