//! Tiny JSON builder and object-field reader for the lab API.
//!
//! No `serde_json`: this tool keeps external deps minimal, and the request bodies are small
//! flat objects.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A JSON value the lab knows how to emit.
#[derive(Debug, Clone)]
pub(super) enum Value {
    /// JSON null.
    Null,
    /// Boolean.
    Bool(bool),
    /// 64-bit integer (all table fields are integers).
    Int(i64),
    /// String.
    Str(String),
    /// Array.
    Array(Vec<Value>),
    /// Object with stable key order.
    Object(BTreeMap<String, Value>),
}

impl Value {
    /// Empty object.
    pub(super) fn object() -> Self {
        Self::Object(BTreeMap::new())
    }

    /// Insert a field into an object. Panics if not an object.
    pub(super) fn insert(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        match self {
            Self::Object(map) => {
                map.insert(key.into(), value.into());
            }
            _ => panic!("insert on non-object"),
        }
    }

    /// Serialize to a compact JSON string.
    pub(super) fn encode(&self) -> String {
        let mut out = String::new();
        self.write_into(&mut out);
        out
    }

    fn write_into(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(true) => out.push_str("true"),
            Self::Bool(false) => out.push_str("false"),
            Self::Int(n) => {
                let _ = write!(out, "{n}");
            }
            Self::Str(s) => write_string(out, s),
            Self::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_into(out);
                }
                out.push(']');
            }
            Self::Object(map) => {
                out.push('{');
                for (i, (key, value)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(out, key);
                    out.push(':');
                    value.write_into(out);
                }
                out.push('}');
            }
        }
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Int(i64::from(v))
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Self::Int(i64::from(v))
    }
}

impl From<usize> for Value {
    fn from(v: usize) -> Self {
        Self::Int(v as i64)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::Str(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Str(v.to_owned())
    }
}

impl From<Vec<Value>> for Value {
    fn from(v: Vec<Value>) -> Self {
        Self::Array(v)
    }
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parsed request body: string and integer fields only (enough for the lab API).
#[derive(Debug, Default, Clone)]
pub(super) struct Request {
    strings: BTreeMap<String, String>,
    ints: BTreeMap<String, i64>,
    bools: BTreeMap<String, bool>,
}

impl Request {
    /// Parse a flat JSON object. Nested values are ignored.
    pub(super) fn parse(input: &str) -> Result<Self, String> {
        let mut parser = Parser::new(input);
        parser.skip_ws();
        parser.expect(b'{')?;
        let mut req = Self::default();
        parser.skip_ws();
        if parser.peek() == Some(b'}') {
            parser.bump();
            return Ok(req);
        }
        loop {
            parser.skip_ws();
            let key = parser.string()?;
            parser.skip_ws();
            parser.expect(b':')?;
            parser.skip_ws();
            match parser.peek() {
                Some(b'"') => {
                    req.strings.insert(key, parser.string()?);
                }
                Some(b't') | Some(b'f') => {
                    req.bools.insert(key, parser.bool()?);
                }
                Some(b'n') => {
                    parser.expect_word("null")?;
                    // null → absent
                }
                Some(b'-') | Some(b'0'..=b'9') => {
                    req.ints.insert(key, parser.int()?);
                }
                Some(b'{') | Some(b'[') => {
                    parser.skip_value()?;
                }
                other => {
                    return Err(format!(
                        "unexpected token in JSON value for `{key}`: {other:?}"
                    ));
                }
            }
            parser.skip_ws();
            match parser.peek() {
                Some(b',') => {
                    parser.bump();
                    continue;
                }
                Some(b'}') => {
                    parser.bump();
                    break;
                }
                other => return Err(format!("expected `,` or `}}`, got {other:?}")),
            }
        }
        Ok(req)
    }

    /// Optional string field.
    pub(super) fn str(&self, key: &str) -> Option<&str> {
        self.strings.get(key).map(String::as_str)
    }

    /// Required string field.
    pub(super) fn require_str(&self, key: &str) -> Result<&str, String> {
        self.str(key)
            .ok_or_else(|| format!("missing string field `{key}`"))
    }

    /// Optional integer field.
    pub(super) fn int(&self, key: &str) -> Option<i64> {
        self.ints.get(key).copied()
    }

    /// Required integer field.
    pub(super) fn require_int(&self, key: &str) -> Result<i64, String> {
        self.int(key)
            .ok_or_else(|| format!("missing integer field `{key}`"))
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.bump();
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        match self.peek() {
            Some(b) if b == byte => {
                self.bump();
                Ok(())
            }
            other => Err(format!("expected '{}', got {:?}", byte as char, other)),
        }
    }

    fn expect_word(&mut self, word: &str) -> Result<(), String> {
        for expected in word.bytes() {
            self.expect(expected)?;
        }
        Ok(())
    }

    fn bool(&mut self) -> Result<bool, String> {
        match self.peek() {
            Some(b't') => {
                self.expect_word("true")?;
                Ok(true)
            }
            Some(b'f') => {
                self.expect_word("false")?;
                Ok(false)
            }
            other => Err(format!("expected boolean, got {other:?}")),
        }
    }

    fn int(&mut self) -> Result<i64, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.bump();
        }
        if !matches!(self.peek(), Some(b'0'..=b'9')) {
            return Err("expected digit".to_owned());
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.bump();
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| "invalid utf-8 in number".to_owned())?;
        text.parse()
            .map_err(|_| format!("invalid integer `{text}`"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err("unterminated string".to_owned()),
                Some(b'"') => {
                    self.bump();
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.bump();
                    match self.peek() {
                        Some(b'"') => {
                            out.push('"');
                            self.bump();
                        }
                        Some(b'\\') => {
                            out.push('\\');
                            self.bump();
                        }
                        Some(b'n') => {
                            out.push('\n');
                            self.bump();
                        }
                        Some(b'r') => {
                            out.push('\r');
                            self.bump();
                        }
                        Some(b't') => {
                            out.push('\t');
                            self.bump();
                        }
                        Some(b'u') => {
                            self.bump();
                            let mut hex = 0u32;
                            for _ in 0..4 {
                                let digit = self.peek().ok_or("short unicode escape")?;
                                self.bump();
                                hex = hex * 16
                                    + u32::from(match digit {
                                        b'0'..=b'9' => digit - b'0',
                                        b'a'..=b'f' => digit - b'a' + 10,
                                        b'A'..=b'F' => digit - b'A' + 10,
                                        _ => return Err("bad unicode escape".to_owned()),
                                    });
                            }
                            out.push(
                                char::from_u32(hex)
                                    .ok_or_else(|| "bad unicode scalar".to_owned())?,
                            );
                        }
                        other => return Err(format!("unknown escape {other:?}")),
                    }
                }
                Some(b) => {
                    out.push(b as char);
                    self.bump();
                }
            }
        }
    }

    fn skip_value(&mut self) -> Result<(), String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.skip_balanced(b'{', b'}'),
            Some(b'[') => self.skip_balanced(b'[', b']'),
            Some(b'"') => {
                let _ = self.string()?;
                Ok(())
            }
            Some(b't') => self.expect_word("true"),
            Some(b'f') => self.expect_word("false"),
            Some(b'n') => self.expect_word("null"),
            Some(b'-') | Some(b'0'..=b'9') => {
                let _ = self.int()?;
                Ok(())
            }
            other => Err(format!("cannot skip value starting {other:?}")),
        }
    }

    fn skip_balanced(&mut self, open: u8, close: u8) -> Result<(), String> {
        self.expect(open)?;
        let mut depth = 1i32;
        let mut in_string = false;
        let mut escape = false;
        while depth > 0 {
            let b = self.peek().ok_or("unterminated structure")?;
            self.bump();
            if in_string {
                if escape {
                    escape = false;
                } else if b == b'\\' {
                    escape = true;
                } else if b == b'"' {
                    in_string = false;
                }
                continue;
            }
            match b {
                b'"' => in_string = true,
                b if b == open => depth += 1,
                b if b == close => depth -= 1,
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_flat_object() {
        let raw = r#"{"family":"C","parameter":"width_ratio.base","value":900,"notes":"ok"}"#;
        let req = Request::parse(raw).unwrap();
        assert_eq!(req.str("family"), Some("C"));
        assert_eq!(req.str("parameter"), Some("width_ratio.base"));
        assert_eq!(req.int("value"), Some(900));
        assert_eq!(req.str("notes"), Some("ok"));
    }

    #[test]
    fn encode_object() {
        let mut v = Value::object();
        v.insert("a", 1);
        v.insert("b", "x");
        assert_eq!(v.encode(), r#"{"a":1,"b":"x"}"#);
    }
}
