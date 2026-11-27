// Bounded protobuf wire primitives shared by request, event and typed-error decoding.
import { ProbeError } from "./transport.mjs";

function varint(value) {
  let number = BigInt(value);
  if (number < 0n || number > 0xffffffffffffffffn) throw new ProbeError("protobuf_integer_range");
  const bytes = [];
  do {
    let byte = Number(number & 127n);
    number >>= 7n;
    if (number) byte |= 128;
    bytes.push(byte);
  } while (number);
  return Buffer.from(bytes);
}

export function field(tag, value) {
  if (!Number.isInteger(tag) || tag < 1 || tag > 0x1fffffff) throw new ProbeError("protobuf_tag");
  if (typeof value === "number") return Buffer.concat([varint(tag * 8), varint(value)]);
  const bytes = typeof value === "string" ? Buffer.from(value, "utf8") : value;
  return Buffer.concat([varint(tag * 8 + 2), varint(bytes.length), bytes]);
}

export function fields(bytes) {
  const result = [];
  let offset = 0;
  const read = () => {
    let value = 0n;
    for (let shift = 0n; shift < 70n; shift += 7n) {
      if (offset >= bytes.length) throw new ProbeError("protobuf_truncated");
      const byte = bytes[offset++];
      if (shift === 63n && byte > 1) throw new ProbeError("protobuf_integer_range");
      value |= BigInt(byte & 127) << shift;
      if (!(byte & 128)) return value;
    }
    throw new ProbeError("protobuf_varint");
  };
  while (offset < bytes.length) {
    if (result.length >= 4096) throw new ProbeError("protobuf_field_limit");
    const key = read(), tag = Number(key >> 3n), wire = Number(key & 7n);
    if (tag < 1 || tag > 0x1fffffff) throw new ProbeError("protobuf_tag");
    let value;
    if (wire === 0) value = read();
    else if ([1, 2, 5].includes(wire)) {
      const size = wire === 2 ? read() : BigInt(wire === 1 ? 8 : 4);
      if (size > BigInt(bytes.length - offset)) throw new ProbeError("protobuf_truncated");
      value = bytes.subarray(offset, offset + Number(size));
      offset += Number(size);
    } else throw new ProbeError("protobuf_wire_type");
    result.push({ tag, wire, value });
  }
  return result;
}

export function frame(bytes, flags = 0) {
  const header = Buffer.alloc(5);
  header[0] = flags;
  header.writeUInt32BE(bytes.length, 1);
  return Buffer.concat([header, bytes]);
}
