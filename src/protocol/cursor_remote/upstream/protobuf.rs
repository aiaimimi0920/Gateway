//! Bounded Protobuf wire, Connect framing, and `google.protobuf.Value` codecs.
use super::ProbeError;
use serde_json::{Map, Number, Value};
use std::collections::HashSet;

const MAX_FIELDS: usize = 4096;
const MAX_TAG: u32 = 0x1fff_ffff;
const MAX_VALUE_DEPTH: usize = 24;
const MAX_SAFE_F64: f64 = 9_007_199_254_740_991.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Field<'a> {
    pub(crate) tag: u32,
    pub(crate) wire: u8,
    pub(crate) varint: u64,
    pub(crate) bytes: &'a [u8],
}

pub(crate) fn varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(10);
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return out;
        }
    }
}

pub(crate) fn field_varint(tag: u32, value: u64) -> Result<Vec<u8>, ProbeError> {
    if tag == 0 || tag > MAX_TAG {
        return Err(ProbeError::new("protobuf_tag"));
    }
    let mut out = varint((u64::from(tag)) << 3);
    out.extend_from_slice(&varint(value));
    Ok(out)
}

pub(crate) fn field_bytes(tag: u32, bytes: &[u8]) -> Result<Vec<u8>, ProbeError> {
    if tag == 0 || tag > MAX_TAG {
        return Err(ProbeError::new("protobuf_tag"));
    }
    let mut out = varint(((u64::from(tag)) << 3) | 2);
    out.extend_from_slice(&varint(bytes.len() as u64));
    out.extend_from_slice(bytes);
    Ok(out)
}

pub(crate) fn field_str(tag: u32, text: &str) -> Result<Vec<u8>, ProbeError> {
    field_bytes(tag, text.as_bytes())
}

pub(crate) fn connect_frame(payload: &[u8], flags: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + payload.len());
    out.push(flags);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

pub(crate) fn fields(bytes: &[u8]) -> Result<Vec<Field<'_>>, ProbeError> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    let read_varint = |offset: &mut usize| -> Result<u64, ProbeError> {
        let mut value = 0u64;
        for shift in (0u32..70).step_by(7) {
            let Some(&byte) = bytes.get(*offset) else {
                return Err(ProbeError::new("protobuf_truncated"));
            };
            *offset += 1;
            if shift == 63 && byte > 1 {
                return Err(ProbeError::new("protobuf_integer_range"));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if (byte & 0x80) == 0 {
                return Ok(value);
            }
        }
        Err(ProbeError::new("protobuf_varint"))
    };
    while offset < bytes.len() {
        if out.len() >= MAX_FIELDS {
            return Err(ProbeError::new("protobuf_field_limit"));
        }
        let key = read_varint(&mut offset)?;
        let tag_u64 = key >> 3;
        let wire = (key & 7) as u8;
        if tag_u64 == 0 || tag_u64 > u64::from(MAX_TAG) {
            return Err(ProbeError::new("protobuf_tag"));
        }
        let tag = tag_u64 as u32;
        match wire {
            0 => {
                let v = read_varint(&mut offset)?;
                out.push(Field {
                    tag,
                    wire,
                    varint: v,
                    bytes: &[],
                });
            }
            1 | 2 | 5 => {
                let size = match wire {
                    2 => usize::try_from(read_varint(&mut offset)?)
                        .map_err(|_| ProbeError::new("protobuf_truncated"))?,
                    1 => 8,
                    _ => 4,
                };
                if size > bytes.len() - offset {
                    return Err(ProbeError::new("protobuf_truncated"));
                }
                let slice = &bytes[offset..offset + size];
                offset += size;
                out.push(Field {
                    tag,
                    wire,
                    varint: 0,
                    bytes: slice,
                });
            }
            _ => return Err(ProbeError::new("protobuf_wire_type")),
        }
    }
    Ok(out)
}

pub(crate) fn oneof<'a>(
    bytes: &'a [u8],
    metadata: &[(u32, u8)],
) -> Result<Option<Field<'a>>, ProbeError> {
    let parsed = fields(bytes)?;
    if parsed.is_empty() {
        return Err(ProbeError::new("unexpected_protobuf_message"));
    }
    let mut seen = HashSet::with_capacity(parsed.len());
    let mut messages = Vec::new();
    for item in parsed {
        if !seen.insert(item.tag) {
            return Err(ProbeError::new("unexpected_protobuf_message"));
        }
        if let Some(&(_, expected_wire)) = metadata.iter().find(|&&(t, _)| t == item.tag) {
            if item.wire != expected_wire {
                return Err(ProbeError::new("unexpected_protobuf_message"));
            }
        } else {
            messages.push(item);
        }
    }
    if messages.len() > 1 || messages.iter().any(|m| m.wire != 2) {
        return Err(ProbeError::new("unexpected_protobuf_message"));
    }
    Ok(messages.into_iter().next())
}

pub(crate) fn encode_value(value: &Value, depth: usize) -> Result<Vec<u8>, ProbeError> {
    let fail = || ProbeError::new("invalid_mcp_message");
    if depth > MAX_VALUE_DEPTH {
        return Err(fail());
    }
    match value {
        Value::Null => field_varint(1, 0),
        Value::Bool(flag) => field_varint(4, u64::from(*flag)),
        Value::String(text) => field_str(3, text),
        Value::Number(num) => {
            let n = num.as_f64().filter(|f| f.is_finite()).ok_or_else(fail)?;
            let mut out = Vec::with_capacity(9);
            out.push(17);
            out.extend_from_slice(&n.to_le_bytes());
            Ok(out)
        }
        Value::Array(items) => {
            let mut buf = Vec::new();
            for item in items {
                buf.extend_from_slice(&field_bytes(1, &encode_value(item, depth + 1)?)?);
            }
            field_bytes(6, &buf)
        }
        Value::Object(map) => {
            let mut buf = Vec::new();
            for (k, v) in map {
                let mut entry = field_str(1, k)?;
                entry.extend_from_slice(&field_bytes(2, &encode_value(v, depth + 1)?)?);
                buf.extend_from_slice(&field_bytes(1, &entry)?);
            }
            field_bytes(5, &buf)
        }
    }
}

pub(crate) fn decode_struct_map(
    bytes: &[u8],
    entry_tag: u32,
    depth: usize,
) -> Result<Map<String, Value>, ProbeError> {
    let fail = || ProbeError::new("invalid_mcp_message");
    let mut map = Map::new();
    for item in fields(bytes)? {
        if item.tag != entry_tag {
            continue;
        }
        if item.wire != 2 {
            return Err(fail());
        }
        let pair = fields(item.bytes)?;
        if pair.len() != 2 {
            return Err(fail());
        }
        let key_bytes = scalar_bytes(&pair, 1).ok_or_else(fail)?;
        let key = utf8_bounded(key_bytes, 256)?;
        if map.contains_key(&key) {
            return Err(fail());
        }
        let val_bytes = scalar_bytes(&pair, 2).ok_or_else(fail)?;
        map.insert(key, decode_value(val_bytes, depth + 1)?);
    }
    Ok(map)
}

fn decode_value(bytes: &[u8], depth: usize) -> Result<Value, ProbeError> {
    let fail = || ProbeError::new("invalid_mcp_message");
    if depth > MAX_VALUE_DEPTH {
        return Err(fail());
    }
    let list = fields(bytes)?;
    let [v] = list.as_slice() else {
        return Err(fail());
    };
    match (v.tag, v.wire) {
        (1, 0) if v.varint == 0 => Ok(Value::Null),
        (2, 1) if v.bytes.len() == 8 => {
            let raw: [u8; 8] = v.bytes.try_into().map_err(|_| fail())?;
            let n = f64::from_le_bytes(raw);
            if !n.is_finite() {
                return Err(fail());
            }
            if n == 0.0 {
                return Ok(Value::Number(Number::from(0)));
            }
            if n.fract() == 0.0 && (-MAX_SAFE_F64..=MAX_SAFE_F64).contains(&n) {
                return Ok(Value::Number(Number::from(n as i64)));
            }
            Number::from_f64(n).map(Value::Number).ok_or_else(fail)
        }
        (3, 2) => utf8_bounded(v.bytes, 65_536).map(Value::String),
        (4, 0) if v.varint <= 1 => Ok(Value::Bool(v.varint == 1)),
        (5, 2) => decode_struct_map(v.bytes, 1, depth + 1).map(Value::Object),
        (6, 2) => {
            let mut arr = Vec::new();
            for item in fields(v.bytes)? {
                if item.tag != 1 || item.wire != 2 {
                    return Err(fail());
                }
                arr.push(decode_value(item.bytes, depth + 1)?);
            }
            Ok(Value::Array(arr))
        }
        _ => Err(fail()),
    }
}

pub(crate) fn scalar_bytes<'a>(raw: &[Field<'a>], tag: u32) -> Option<&'a [u8]> {
    let mut iter = raw.iter().filter(|f| f.tag == tag);
    let first = iter.next()?;
    if iter.next().is_some() || first.wire != 2 {
        return None;
    }
    Some(first.bytes)
}

pub(crate) fn scalar_varint_opt(
    raw: &[Field<'_>],
    tag: u32,
    fallback: u64,
) -> Result<u64, ProbeError> {
    let mut iter = raw.iter().filter(|f| f.tag == tag);
    let Some(first) = iter.next() else {
        return Ok(fallback);
    };
    if iter.next().is_some() || first.wire != 0 {
        return Err(ProbeError::new("invalid_mcp_message"));
    }
    Ok(first.varint)
}

pub(crate) fn utf8_bounded(bytes: &[u8], max: usize) -> Result<String, ProbeError> {
    if bytes.len() > max {
        return Err(ProbeError::new("invalid_mcp_message"));
    }
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| ProbeError::new("invalid_mcp_message"))
}
