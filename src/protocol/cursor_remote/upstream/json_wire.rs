//! Strict JSON and ProtoJSON wire decoders matching pinned Cursor RPC contracts.
//! Rejects duplicate keys, deep nesting, non-canonical integer tokens, and null scalars.
use super::ProbeError;
use serde_json::{Map, Number, Value};
use std::collections::{HashMap, HashSet};

const MAX_JSON_BYTES: usize = 512 * 1024;
const MAX_JSON_DEPTH: usize = 64;
const MAX_SAFE_INT: i64 = 9_007_199_254_740_991;
const MIN_SAFE_INT: i64 = -9_007_199_254_740_991;

pub(crate) fn parse_json(
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<Map<String, Value>, ProbeError> {
    if status != 200 {
        return Err(ProbeError::with_status("upstream_http_error", status));
    }
    if content_type != "application/json" {
        return Err(ProbeError::new("unexpected_json_content_type"));
    }
    match unique_json(body)? {
        Value::Object(map) => Ok(map),
        _ => Err(ProbeError::new("invalid_json_response")),
    }
}

pub(crate) fn unique_json(bytes: &[u8]) -> Result<Value, ProbeError> {
    let invalid = || ProbeError::new("invalid_json_response");
    if bytes.len() > MAX_JSON_BYTES {
        return Err(invalid());
    }
    let raw = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    let text = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);
    let mut parser = JsonParser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    let value = parser.parse_value(0).ok_or_else(invalid)?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(invalid());
    }
    Ok(value)
}

pub(crate) fn camel_to_snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

pub(crate) fn wire_fields<'a>(
    obj: &'a Map<String, Value>,
    allowed_camel: &[&'static str],
) -> Result<HashMap<&'static str, &'a Value>, ProbeError> {
    let mut allowed: HashMap<String, &'static str> =
        HashMap::with_capacity(allowed_camel.len() * 2);
    for &name in allowed_camel {
        allowed.insert(name.to_owned(), name);
        allowed.insert(camel_to_snake(name), name);
    }
    let mut out: HashMap<&'static str, &'a Value> = HashMap::with_capacity(obj.len());
    for (key, value) in obj {
        let Some(&canonical) = allowed.get(key.as_str()) else {
            return Err(ProbeError::new("unknown_or_duplicate_protojson_field"));
        };
        if out.contains_key(canonical) {
            return Err(ProbeError::new("unknown_or_duplicate_protojson_field"));
        }
        if value.is_null() {
            return Err(ProbeError::new("null_protojson_field"));
        }
        out.insert(canonical, value);
    }
    Ok(out)
}

pub(crate) fn proto_field<'a>(
    obj: &'a Map<String, Value>,
    camel: &str,
    err_code: &'static str,
) -> Result<Option<&'a Value>, ProbeError> {
    let snake = camel_to_snake(camel);
    if snake != camel && obj.contains_key(camel) && obj.contains_key(&snake) {
        return Err(ProbeError::new(err_code));
    }
    Ok(obj.get(camel).or_else(|| obj.get(&snake)))
}

pub(crate) fn scalar_bool_opt(
    fields: &HashMap<&'static str, &Value>,
    name: &'static str,
) -> Result<Option<bool>, ProbeError> {
    let Some(value) = fields.get(name) else {
        return Ok(None);
    };
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| ProbeError::new("invalid_protojson_scalar"))
}

pub(crate) fn scalar_bool(
    fields: &HashMap<&'static str, &Value>,
    name: &'static str,
    absent: bool,
) -> Result<bool, ProbeError> {
    Ok(scalar_bool_opt(fields, name)?.unwrap_or(absent))
}

pub(crate) fn scalar_i32_opt(
    fields: &HashMap<&'static str, &Value>,
    name: &'static str,
) -> Result<Option<i32>, ProbeError> {
    let Some(value) = scalar_safe_int(fields, name)? else {
        return Ok(None);
    };
    i32::try_from(value)
        .map(Some)
        .map_err(|_| ProbeError::new("invalid_protojson_integer"))
}

pub(crate) fn scalar_i32(
    fields: &HashMap<&'static str, &Value>,
    name: &'static str,
    absent: i32,
) -> Result<i32, ProbeError> {
    Ok(scalar_i32_opt(fields, name)?.unwrap_or(absent))
}

pub(crate) fn scalar_i64_safe(
    fields: &HashMap<&'static str, &Value>,
    name: &'static str,
    absent: i64,
) -> Result<i64, ProbeError> {
    Ok(scalar_safe_int(fields, name)?.unwrap_or(absent))
}

fn scalar_safe_int(
    fields: &HashMap<&'static str, &Value>,
    name: &'static str,
) -> Result<Option<i64>, ProbeError> {
    let Some(item) = fields.get(name) else {
        return Ok(None);
    };
    let parsed = match item {
        Value::Number(num) => num
            .as_i64()
            .filter(|&n| (MIN_SAFE_INT..=MAX_SAFE_INT).contains(&n)),
        Value::String(text) if is_canonical_int_string(text, 19) => text
            .parse::<i64>()
            .ok()
            .filter(|&n| (MIN_SAFE_INT..=MAX_SAFE_INT).contains(&n)),
        _ => None,
    };
    parsed
        .map(Some)
        .ok_or_else(|| ProbeError::new("invalid_protojson_integer"))
}

pub(crate) fn is_canonical_int_string(text: &str, max_digits: usize) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || digits.len() > max_digits {
        return false;
    }
    if digits == "0" {
        return text != "-0";
    }
    let mut bytes = digits.bytes();
    matches!(bytes.next(), Some(b'1'..=b'9')) && bytes.all(|b| b.is_ascii_digit())
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\r' | b'\n')
        {
            self.pos += 1;
        }
    }

    fn parse_value(&mut self, depth: usize) -> Option<Value> {
        self.skip_ws();
        let ch = *self.bytes.get(self.pos)?;
        match ch {
            b'{' => self.parse_object(depth),
            b'[' => self.parse_array(depth),
            b'"' => self.parse_string().map(Value::String),
            b't' if self.bytes[self.pos..].starts_with(b"true") => {
                self.pos += 4;
                Some(Value::Bool(true))
            }
            b'f' if self.bytes[self.pos..].starts_with(b"false") => {
                self.pos += 5;
                Some(Value::Bool(false))
            }
            b'n' if self.bytes[self.pos..].starts_with(b"null") => {
                self.pos += 4;
                Some(Value::Null)
            }
            b'-' | b'0'..=b'9' => self.parse_number(),
            _ => None,
        }
    }

    fn parse_object(&mut self, depth: usize) -> Option<Value> {
        let next = depth + 1;
        if next > MAX_JSON_DEPTH {
            return None;
        }
        self.pos += 1;
        self.skip_ws();
        let mut map = Map::new();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Some(Value::Object(map));
        }
        let mut seen = HashSet::new();
        loop {
            self.skip_ws();
            if self.bytes.get(self.pos) != Some(&b'"') {
                return None;
            }
            let key = self.parse_string()?;
            if !seen.insert(key.clone()) {
                return None;
            }
            self.skip_ws();
            if self.bytes.get(self.pos) != Some(&b':') {
                return None;
            }
            self.pos += 1;
            let val = self.parse_value(next)?;
            map.insert(key, val);
            self.skip_ws();
            match *self.bytes.get(self.pos)? {
                b',' => self.pos += 1,
                b'}' => {
                    self.pos += 1;
                    return Some(Value::Object(map));
                }
                _ => return None,
            }
        }
    }

    fn parse_array(&mut self, depth: usize) -> Option<Value> {
        let next = depth + 1;
        if next > MAX_JSON_DEPTH {
            return None;
        }
        self.pos += 1;
        self.skip_ws();
        let mut items = Vec::new();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Some(Value::Array(items));
        }
        loop {
            items.push(self.parse_value(next)?);
            self.skip_ws();
            match *self.bytes.get(self.pos)? {
                b',' => self.pos += 1,
                b']' => {
                    self.pos += 1;
                    return Some(Value::Array(items));
                }
                _ => return None,
            }
        }
    }

    fn parse_string(&mut self) -> Option<String> {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b'"' => {
                    self.pos += 1;
                    return serde_json::from_slice(&self.bytes[start..self.pos]).ok();
                }
                b'\\' => {
                    self.pos += 2;
                }
                0x00..=0x1f => return None,
                _ => self.pos += 1,
            }
        }
        None
    }

    fn parse_number(&mut self) -> Option<Value> {
        let start = self.pos;
        if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        match *self.bytes.get(self.pos)? {
            b'0' => self.pos += 1,
            b'1'..=b'9' => {
                self.pos += 1;
                while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                    self.pos += 1;
                }
            }
            _ => return None,
        }
        let mut has_frac_or_exp = false;
        if self.bytes.get(self.pos) == Some(&b'.') {
            has_frac_or_exp = true;
            self.pos += 1;
            if !self.bytes.get(self.pos).is_some_and(u8::is_ascii_digit) {
                return None;
            }
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        if matches!(self.bytes.get(self.pos), Some(b'e' | b'E')) {
            has_frac_or_exp = true;
            self.pos += 1;
            if matches!(self.bytes.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !self.bytes.get(self.pos).is_some_and(u8::is_ascii_digit) {
                return None;
            }
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let token = std::str::from_utf8(&self.bytes[start..self.pos]).ok()?;
        let float: f64 = token.parse().ok().filter(|n: &f64| n.is_finite())?;
        if float.fract() == 0.0 {
            if has_frac_or_exp {
                return None;
            }
            let integer: i64 = token
                .parse()
                .ok()
                .filter(|&n| (MIN_SAFE_INT..=MAX_SAFE_INT).contains(&n))?;
            return Some(Value::Number(Number::from(integer)));
        }
        Number::from_f64(float).map(Value::Number)
    }
}
