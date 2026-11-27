// Text-only Connect session control, from the pinned official CLI descriptors.
// Context is static and blobs stay in bounded request-local memory. No execution,
// filesystem, workspace, network tools, history import or server-directed actions.
import { field, fields, frame } from "./protobuf_wire.mjs";
import { ProbeError } from "./transport.mjs";

const reject = () => { throw new ProbeError("remote_client_action_required"); };
function selected(raw, allowed) {
  const result = new Map();
  for (const item of fields(raw)) {
    if (!allowed.includes(item.tag) || result.has(item.tag)) reject();
    result.set(item.tag, item);
  }
  return result;
}
function bytes(item, max) {
  if (!item || item.wire !== 2 || item.value.length > max) reject();
  return item.value;
}
function number(item) {
  if (!item) return 0;
  if (item.wire !== 0 || item.value > 0xffffffffn) reject();
  return Number(item.value);
}

export class TextSession {
  constructor(context = Buffer.concat([field(17, 0), field(24, 0)])) {
    this.requestContext = context;
  }
  blobs = new Map();
  storedBytes = 0;
  messages = 0;
  contexts = 0;
  outboundBytes = 0;
  closed = false;

  reply(envelope) {
    if (this.closed || ++this.messages > 128) reject();
    const response = envelope.tag === 2 ? this.context(envelope.value)
      : envelope.tag === 4 ? this.blob(envelope.value) : reject();
    // Cap before the decoder can queue repeated large get-blob replies from one chunk.
    if ((this.outboundBytes += response.length) > 320 * 1024) reject();
    return response;
  }

  context(raw) {
    if (++this.contexts > 4) reject();
    const values = selected(raw, [1, 10, 15, 19, 57]);
    const args = selected(bytes(values.get(10), 4096), [2, 3, 4, 5, 7, 8]);
    // Validate metadata shape, but never use it as paths, commands or cache keys.
    for (const item of args.values()) {
      if ([7, 8].includes(item.tag)) {
        if (number(item) > 1) reject();
      } else bytes(item, 1024);
    }
    if (values.has(19)) bytes(values.get(19), 4096);
    if (values.has(57)) bytes(values.get(57), 256);
    const ids = [field(1, number(values.get(1)))];
    if (values.has(15)) {
      const id = bytes(values.get(15), 256);
      try { new TextDecoder("utf-8", { fatal: true }).decode(id); } catch { reject(); }
      ids.push(field(15, id));
    }
    const context = this.requestContext;
    // AgentClientMessage.exec_client_message / request_context_result / success.
    return frame(field(2, Buffer.concat([...ids, field(10, field(1, field(1, context)))])));
  }

  blob(raw) {
    const values = selected(raw, [1, 2, 3, 4]);
    if (values.has(2) === values.has(3)) reject();
    if (values.has(4)) bytes(values.get(4), 4096);
    const set = values.has(3), args = selected(bytes(values.get(set ? 3 : 2), 260 * 1024), set ? [1, 2] : [1]);
    const id = bytes(args.get(1), 32);
    if (id.length !== 32) reject();
    const key = id.toString("hex"), stored = this.blobs.get(key);
    let result;
    if (set) {
      const value = args.has(2) ? bytes(args.get(2), 256 * 1024) : Buffer.alloc(0);
      if (stored && !stored.equals(value)) reject();
      if (!stored) {
        if (this.blobs.size >= 64 || this.storedBytes + value.length > 256 * 1024) reject();
        this.blobs.set(key, Buffer.from(value));
        this.storedBytes += value.length;
      }
      result = field(3, Buffer.alloc(0));
    } else {
      if (!stored) reject();
      result = field(2, field(1, stored));
    }
    return frame(field(3, Buffer.concat([field(1, number(values.get(1))), result])));
  }

  close() {
    this.closed = true;
    for (const value of this.blobs.values()) value.fill(0);
    this.blobs.clear();
    this.storedBytes = 0;
  }
}
