const MAX_RECORDS = 4096;
const MAX_RECORD_BYTES = 1024 * 1024;
const MAX_CAPTURE_BYTES = 16 * 1024 * 1024;

function exceeded() {
  return Object.assign(new Error("AI Studio browser capture budget exceeded."), {
    code: "aistudio_probe_capture_budget_exceeded", status: 413,
  });
}

function recordBytes(record) {
  const pending = [{ value: record, depth: 0 }];
  let scheduled = 1;
  let rawBytes = 0;
  while (pending.length) {
    const { value, depth } = pending.pop();
    if (depth > 8) throw exceeded();
    if (typeof value === "string") rawBytes += Buffer.byteLength(value, "utf8");
    else if (value && typeof value === "object") {
      for (const key in value) {
        if (!Object.hasOwn(value, key)) continue;
        if (++scheduled > 2048) throw exceeded();
        rawBytes += Buffer.byteLength(key, "utf8");
        if (rawBytes > MAX_RECORD_BYTES) throw exceeded();
        pending.push({ value: value[key], depth: depth + 1 });
      }
    }
    if (rawBytes > MAX_RECORD_BYTES) throw exceeded();
  }
  // The preflight bounds traversal and raw strings before JSON escaping can
  // allocate a serialized copy. The final charge includes escaping/punctuation.
  const bytes = Buffer.byteLength(JSON.stringify(record), "utf8") + 2;
  if (bytes > MAX_RECORD_BYTES) throw exceeded();
  return bytes;
}

export function createCaptureBudget() {
  let records = 0;
  let bytes = 0;
  const objectBytes = new WeakMap();
  return (entries, record, owner = null) => {
    if (records >= MAX_RECORDS) throw exceeded();
    const size = recordBytes(record);
    if (size > MAX_CAPTURE_BYTES - bytes) throw exceeded();
    const ownerBytes = owner ? objectBytes.get(owner) : null;
    if (owner && (ownerBytes === undefined || size > MAX_RECORD_BYTES - ownerBytes)) throw exceeded();
    entries.push(record);
    if (owner) objectBytes.set(owner, ownerBytes + size);
    if (record && typeof record === "object") objectBytes.set(record, size);
    records++;
    bytes += size;
  };
}
