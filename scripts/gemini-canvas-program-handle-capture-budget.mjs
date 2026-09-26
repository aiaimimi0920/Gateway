const LIMITS = Object.freeze({
  inputBytes: 4 * 1024 * 1024,
  sourceBytes: 64 * 1024 * 1024,
  recordBytes: 8 * 1024 * 1024,
  retainedBytes: 64 * 1024 * 1024,
  records: 1024,
  pendingReads: 8,
});

const LIMIT_CODE = "gateway_program_handle_capture_limit_exceeded";
function exceeded(reason) {
  return Object.assign(new Error(`Program-handle capture exceeded its ${reason} limit.`), {
    code: LIMIT_CODE,
  });
}

// Evidence processing/retention budgets complement the CDP transport's pre-read
// decoded-body and protocol-envelope admission checks.
export function createProgramCaptureBudget(overrides = {}) {
  const limits = { ...LIMITS };
  for (const [name, value] of Object.entries(overrides)) {
    if (!Object.hasOwn(limits, name) || !Number.isSafeInteger(value) || value < 1 || value > limits[name]) {
      throw new TypeError("Capture limit overrides must lower known positive integer limits.");
    }
    limits[name] = value;
  }
  let sourceBytes = 0, retainedBytes = 0, records = 0, pendingReads = 0, failure = null;
  const assertHealthy = () => { if (failure) throw failure; };
  return {
    assertHealthy,
    fail(error) {
      failure ??= error?.code === LIMIT_CODE ? error : Object.assign(
        new Error("Program-handle network evidence capture failed."),
        { code: "gateway_program_handle_capture_failed" },
      );
    },
    inspectSource(...values) {
      assertHealthy();
      let bytes = 0;
      for (const value of values) {
        if (typeof value !== "string") continue;
        if (value.length > limits.inputBytes) throw exceeded("input_bytes");
        const size = Buffer.byteLength(value, "utf8");
        if (size > limits.inputBytes) throw exceeded("input_bytes");
        bytes += size;
      }
      if (bytes > limits.sourceBytes - sourceBytes) throw exceeded("source_bytes");
      sourceBytes += bytes;
    },
    accept(record) {
      assertHealthy();
      if (records >= limits.records) throw exceeded("records");
      const pending = [{ value: record, depth: 0 }];
      let nodes = 1, rawBytes = 0;
      while (pending.length) {
        const { value, depth } = pending.pop();
        if (depth > 8) throw exceeded("record_structure");
        if (typeof value === "string") rawBytes += Buffer.byteLength(value, "utf8");
        else if (value && typeof value === "object") {
          for (const key in value) {
            if (!Object.hasOwn(value, key)) continue;
            if (++nodes > 2048) throw exceeded("record_structure");
            rawBytes += Buffer.byteLength(key, "utf8");
            pending.push({ value: value[key], depth: depth + 1 });
          }
        }
        if (rawBytes > limits.recordBytes) throw exceeded("record_bytes");
      }
      // Bound strings and traversal before allocating the escaped JSON copy.
      const bytes = Buffer.byteLength(JSON.stringify(record), "utf8");
      if (bytes > limits.recordBytes) throw exceeded("record_bytes");
      if (bytes > limits.retainedBytes - retainedBytes) throw exceeded("retained_bytes");
      retainedBytes += bytes;
      records++;
    },
    beginRead() {
      assertHealthy();
      if (pendingReads >= limits.pendingReads) throw exceeded("pending_reads");
      pendingReads++;
      let released = false;
      return () => {
        if (!released) { released = true; pendingReads--; }
      };
    },
  };
}
