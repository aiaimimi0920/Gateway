import { EventEmitter } from "node:events";

const failure = () => Object.assign(new Error("Program-handle response capture session failed."), {
  code: "gateway_program_handle_capture_failed",
});
const childRoutes = new WeakMap();

// Pages use direct browser routes; worker/frame sessions retain their CDP ancestry.
class ChildSession extends EventEmitter {
  constructor(transport, sessionId, parent, onError, timeoutMs) {
    super();
    Object.assign(this, { transport, sessionId, parent, onError, timeoutMs });
    this.nestingDepth = transport instanceof ChildSession ? transport.nestingDepth + 1 : 0;
    this.pending = new Map();
    this.nextId = 0;
    this.closed = false;
    this.receive = (event) => {
      if (this.closed || event.sessionId !== sessionId) return;
      try {
        if (event.message.length > 32 * 1024 * 1024) throw failure();
        const reply = JSON.parse(event.message);
        if (Number.isSafeInteger(reply.id)) {
          const pending = this.pending.get(reply.id);
          if (!pending) return;
          this.pending.delete(reply.id);
          clearTimeout(pending.timer);
          if (reply.error) pending.reject(failure());
          else pending.resolve(reply.result);
        } else if (typeof reply.method === "string") this.emit(reply.method, reply.params);
      } catch { onError(failure()); }
    };
    let router = childRoutes.get(transport);
    if (!router) {
      const children = new Map();
      router = { children, receive: (event) => children.get(event?.sessionId)?.receive(event) };
      childRoutes.set(transport, router);
      transport.on("Target.receivedMessageFromTarget", router.receive);
    }
    router.children.set(sessionId, this);
  }

  send(method, params) {
    if (this.closed) return Promise.reject(failure());
    if (this.pending.size >= 16) { this.onError(failure()); return Promise.reject(failure()); }
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      const finishError = () => {
        const pending = this.pending.get(id);
        if (!pending) return;
        this.pending.delete(id);
        clearTimeout(pending.timer);
        reject(failure());
        this.onError(failure());
      };
      const timer = setTimeout(finishError, this.timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      void Promise.resolve().then(() => {
        if (this.closed || !this.pending.has(id)) return;
        return this.transport.send("Target.sendMessageToTarget", {
          sessionId: this.sessionId, message: JSON.stringify({ id, method, params }),
        });
      }).catch(finishError);
    });
  }

  dispose() {
    if (this.closed) return;
    this.closed = true;
    const router = childRoutes.get(this.transport);
    router.children.delete(this.sessionId);
    if (!router.children.size) {
      this.transport.off("Target.receivedMessageFromTarget", router.receive);
      childRoutes.delete(this.transport);
    }
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(failure());
    }
    this.pending.clear();
    this.removeAllListeners();
  }
}

// One context owns all adopted pages before their first request is allowed to run.
export function createProgramCdpSessions(page, { configure, onError, timeoutMs = 5000, pageOnly = false }) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 5000) {
    throw new TypeError("Session timeout must lower the positive default limit.");
  }
  const entries = new Map(), targets = new Map(), tasks = new Set(), controls = new Set(), transients = new Set();
  let root = null, identity = null, contextId, selectedTargetId, stopped = false, stopping = null, failed = false;
  let live = 0, created = 0, detaching = null, detachingIdentity = null;
  const bounded = (promise, cancelOnStop = true) => new Promise((resolve, reject) => {
    let settled = false;
    const finish = (ok, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      controls.delete(cancel);
      if (ok) resolve(value); else reject(failure());
    };
    const cancel = () => finish(false);
    const timer = setTimeout(cancel, timeoutMs);
    if (cancelOnStop) controls.add(cancel);
    Promise.resolve(promise).then((value) => finish(true, value), cancel);
    if (cancelOnStop && stopped) cancel();
  });
  const detachRoot = () => {
    if (!root) return Promise.resolve();
    detaching ??= bounded(Promise.resolve().then(() => root.detach()), false).catch(() => undefined);
    return detaching;
  };
  const detachIdentity = () => {
    if (!identity) return Promise.resolve();
    detachingIdentity ??= bounded(Promise.resolve().then(() => identity.detach()), false).catch(() => undefined);
    return detachingIdentity;
  };
  const report = () => {
    if (stopped || failed) return;
    failed = true;
    void stop();
    onError(failure());
  };
  const claim = (count) => {
    if (live + count > 16 || created + count > 256) throw failure();
    live += count;
    created += count;
    let released = false;
    return () => { if (!released) { released = true; live -= count; } };
  };
  const remove = (session) => {
    const entry = entries.get(session);
    if (!entry) return;
    entries.delete(session);
    entry.closed = true;
    targets.delete(entry.targetId);
    for (const child of [...entries.keys()]) if (child.transport === session) remove(child);
    session.off("Target.attachedToTarget", entry.attached);
    session.off("Target.detachedFromTarget", entry.detached);
    entry.controller.abort();
    try { entry.dispose?.(); } catch { report(); }
    session.dispose();
    entry.release();
  };
  const detached = (parent, { sessionId }) => {
    for (const [session] of entries) {
      if (session.transport === parent && session.sessionId === sessionId) { remove(session); return; }
    }
    for (const entry of targets.values()) {
      if ((parent === root && entry.directId === sessionId) ||
          (entry.parent === parent && entry.autoId === sessionId && !entry.releasingAuto)) {
        entry.closed = true;
      }
    }
  };
  const register = async (session, entry) => {
    entry.attached = (event) => attached(session, event);
    entry.detached = (event) => detached(session, event);
    session.on("Target.attachedToTarget", entry.attached);
    session.on("Target.detachedFromTarget", entry.detached);
    // Abort removes network listeners before a pending configure command can finish.
    await bounded(Promise.resolve().then(() => {
      if (stopped || entry.closed) return () => undefined;
      return configure(session, entry.controller.signal);
    }).then((dispose) => {
      if (stopped || entry.closed) { dispose(); return; }
      entry.dispose = dispose;
    }));
    if (stopped || entry.closed) return;
    await bounded(session.send("Target.setAutoAttach", { autoAttach: true, waitForDebuggerOnStart: true, flatten: false }));
  };
  function attached(parent, event) {
    if (stopped) return;
    const { sessionId, targetInfo } = event;
    const targetId = targetInfo?.targetId;
    const existing = targets.get(targetId);
    // attachToTarget announces the direct session before returning its ID.
    if (parent === root && existing?.attachingDirect) {
      existing.directId = sessionId;
      return;
    }
    let releaseAuto, releaseDirect;
    try {
      if (typeof sessionId !== "string" || !sessionId || typeof targetId !== "string" || !targetId || existing) throw failure();
      releaseAuto = claim(1);
      const selected = parent === root
        ? targetInfo.type === "page" && (targetInfo.browserContextId ?? null) === contextId &&
          (!pageOnly || targetInfo.targetId === selectedTargetId)
        : ["iframe", "worker"].includes(targetInfo.type);
      const direct = selected && parent === root;
      if (direct) releaseDirect = claim(1);
      const entry = { targetId, parent, autoId: sessionId, directId: null, attachingDirect: direct,
        releasingAuto: false, closed: false, controller: new AbortController(), dispose: null, release: releaseDirect ?? releaseAuto };
      targets.set(targetId, entry);
      const task = (async () => {
        let session = null, transferred = false;
        try {
          if (!selected && parent === root) {
            entry.releasingAuto = true;
            await bounded(root.send("Target.detachFromTarget", { sessionId }));
            return;
          }
          if (direct) {
            const result = await bounded(root.send("Target.attachToTarget", { targetId, flatten: false }));
            entry.attachingDirect = false;
            if (stopped || entry.closed) return;
            if (typeof result.sessionId !== "string" || !result.sessionId ||
                (entry.directId && entry.directId !== result.sessionId)) throw failure();
            entry.directId = result.sessionId;
            session = new ChildSession(root, result.sessionId, parent, report, timeoutMs);
          } else {
            session = new ChildSession(parent, sessionId, parent, report, timeoutMs);
          }
          if (selected) {
            entries.set(session, entry);
            transferred = true;
            await register(session, entry);
            if (stopped || entry.closed) return;
          } else transients.add(session);
          await bounded(session.send("Runtime.runIfWaitingForDebugger"));
          if (stopped || entry.closed || (selected && !direct)) return;
          entry.releasingAuto = true;
          await bounded(parent.send("Target.detachFromTarget", { sessionId }));
        } catch { if (!stopped && !entry.closed) report(); }
        finally {
          if (direct || !transferred) releaseAuto();
          if (!transferred) { releaseDirect?.(); targets.delete(targetId); }
          if (!selected) { session?.dispose(); transients.delete(session); }
        }
      })();
      tasks.add(task);
      void task.then(() => tasks.delete(task), () => { tasks.delete(task); report(); });
    } catch { releaseAuto?.(); releaseDirect?.(); report(); }
  }
  const rootAttached = (event) => attached(root, event);
  const rootDetached = (event) => detached(root, event);
  const ready = (async () => {
    try {
      await bounded(Promise.resolve().then(() => page.context().newCDPSession(page)).then(async (session) => {
        identity = session;
        if (stopped) await detachIdentity();
      }));
      if (stopped) return;
      const { targetInfo } = await bounded(identity.send("Target.getTargetInfo"));
      if (!targetInfo || (targetInfo.browserContextId !== undefined && typeof targetInfo.browserContextId !== "string")) throw failure();
      contextId = targetInfo.browserContextId ?? null;
      selectedTargetId = targetInfo.targetId;
      if (pageOnly && (typeof selectedTargetId !== "string" || !selectedTargetId)) throw failure();
      await detachIdentity();
      if (stopped) return;
      await bounded(Promise.resolve().then(() => page.context().browser().newBrowserCDPSession()).then(async (session) => {
        root = session;
        if (stopped) await detachRoot();
      }));
      if (stopped) return;
      claim(1);
      root.on("Target.attachedToTarget", rootAttached);
      root.on("Target.detachedFromTarget", rootDetached);
      root.on("close", report);
      // Chromium requires flatten here. The automatic session holds the page
      // paused until its public non-flattened capture session is configured.
      await bounded(root.send("Target.setAutoAttach", { autoAttach: true, waitForDebuggerOnStart: true,
        flatten: true, filter: [{ type: "page" }, { exclude: true }] }));
      while (tasks.size && !stopped) await Promise.all([...tasks]);
      if (failed) throw failure();
    } catch { report(); throw failure(); }
  })();
  void ready.catch(() => undefined);
  async function waitForTargets() {
    // Page.navigate can block debugger resume if queued during newPage startup.
    await ready;
    while (tasks.size && !stopped) await Promise.all([...tasks]);
    if (failed || stopped) throw failure();
  }
  function stop() {
    if (stopping) return stopping;
    stopped = true;
    for (const cancel of [...controls]) cancel();
    for (const entry of targets.values()) entry.closed = true;
    for (const session of [...entries.keys()]) remove(session);
    for (const session of transients) session.dispose();
    transients.clear();
    root?.off("Target.attachedToTarget", rootAttached);
    root?.off("Target.detachedFromTarget", rootDetached);
    root?.off("close", report);
    stopping = (async () => {
      await Promise.all([detachRoot(), detachIdentity()]);
      await ready.catch(() => undefined);
      await Promise.all([...tasks]);
      targets.clear();
    })();
    return stopping;
  }
  return { ready, stop, waitForTargets };
}
