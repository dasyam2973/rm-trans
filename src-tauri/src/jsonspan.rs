//! 각 값의 원본 바이트 구간을 기억하는 최소한의 JSON 파서.
//!
//! 내보내기 시 파일 전체를 다시 직렬화하지 않고 번역된 문자열 구간만 교체해서
//! 원본의 키 순서, 공백, 숫자 표기를 그대로 보존하기 위해 사용한다.

use std::fmt;

#[derive(Debug)]
pub struct Node {
    /// 값의 시작 바이트 위치 (문자열이면 여는 따옴표 위치)
    pub start: usize,
    /// 값의 끝 바이트 위치 (exclusive, 문자열이면 닫는 따옴표 다음)
    pub end: usize,
    pub value: Value,
}

#[derive(Debug)]
pub enum Value {
    Null,
    #[allow(dead_code)]
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

#[derive(Debug)]
pub struct ParseError {
    pub pos: usize,
    pub msg: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON 파싱 오류 ({}바이트): {}", self.pos, self.msg)
    }
}

impl Node {
    pub fn get(&self, key: &str) -> Option<&Node> {
        match &self.value {
            Value::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn at(&self, index: usize) -> Option<&Node> {
        self.as_array().and_then(|a| a.get(index))
    }

    pub fn as_str(&self) -> Option<&str> {
        match &self.value {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self.value {
            Value::Number(n) if n.fract() == 0.0 => Some(n as i64),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Node]> {
        match &self.value {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Node)]> {
        match &self.value {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// RFC 6901 JSON Pointer로 하위 노드를 찾는다.
    pub fn pointer(&self, pointer: &str) -> Option<&Node> {
        if pointer.is_empty() {
            return Some(self);
        }
        let rest = pointer.strip_prefix('/')?;
        let mut node = self;
        for token in rest.split('/') {
            let token = token.replace("~1", "/").replace("~0", "~");
            node = match &node.value {
                Value::Object(_) => node.get(&token)?,
                Value::Array(_) => node.at(token.parse().ok()?)?,
                _ => return None,
            };
        }
        Some(node)
    }
}

/// JSON Pointer 토큰 이스케이프
pub fn escape_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

pub fn parse(src: &str) -> Result<Node, ParseError> {
    let mut p = Parser { src: src.as_bytes(), text: src, pos: 0 };
    // UTF-8 BOM 건너뛰기
    if src.starts_with('\u{feff}') {
        p.pos = 3;
    }
    p.skip_ws();
    let node = p.value()?;
    p.skip_ws();
    if p.pos != p.src.len() {
        return Err(p.err("값 뒤에 불필요한 데이터가 있음"));
    }
    Ok(node)
}

struct Parser<'a> {
    src: &'a [u8],
    text: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, msg: &'static str) -> ParseError {
        ParseError { pos: self.pos, msg }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, b: u8, msg: &'static str) -> Result<(), ParseError> {
        if self.peek() == Some(b) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err(msg))
        }
    }

    fn literal(&mut self, lit: &[u8], value: Value) -> Result<Value, ParseError> {
        if self.src[self.pos..].starts_with(lit) {
            self.pos += lit.len();
            Ok(value)
        } else {
            Err(self.err("알 수 없는 리터럴"))
        }
    }

    fn value(&mut self) -> Result<Node, ParseError> {
        let start = self.pos;
        let value = match self.peek() {
            Some(b'{') => self.object()?,
            Some(b'[') => self.array()?,
            Some(b'"') => Value::Str(self.string()?),
            Some(b't') => self.literal(b"true", Value::Bool(true))?,
            Some(b'f') => self.literal(b"false", Value::Bool(false))?,
            Some(b'n') => self.literal(b"null", Value::Null)?,
            Some(b'-' | b'0'..=b'9') => self.number()?,
            Some(_) => return Err(self.err("예상하지 못한 문자")),
            None => return Err(self.err("예상하지 못한 파일 끝")),
        };
        Ok(Node { start, end: self.pos, value })
    }

    fn object(&mut self) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(fields));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("객체 키가 필요함"));
            }
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':', "':'가 필요함")?;
            self.skip_ws();
            let val = self.value()?;
            fields.push((key, val));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(fields));
                }
                _ => return Err(self.err("',' 또는 '}'가 필요함")),
            }
        }
    }

    fn array(&mut self) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.err("',' 또는 ']'가 필요함")),
            }
        }
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        while let Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
        self.text[start..self.pos]
            .parse::<f64>()
            .map(Value::Number)
            .map_err(|_| ParseError { pos: start, msg: "잘못된 숫자" })
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let s = self.text.get(self.pos..self.pos + 4).ok_or_else(|| self.err("잘못된 \\u 이스케이프"))?;
        let v = u32::from_str_radix(s, 16).map_err(|_| self.err("잘못된 \\u 이스케이프"))?;
        self.pos += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.pos += 1; // 여는 따옴표
        let mut out = String::new();
        let mut run = self.pos;
        loop {
            match self.peek() {
                None => return Err(self.err("닫히지 않은 문자열")),
                Some(b'"') => {
                    out.push_str(&self.text[run..self.pos]);
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    out.push_str(&self.text[run..self.pos]);
                    self.pos += 1;
                    let esc = self.peek().ok_or_else(|| self.err("닫히지 않은 문자열"))?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&hi) && self.src[self.pos..].starts_with(b"\\u") {
                                self.pos += 2;
                                let lo = self.hex4()?;
                                0x10000 + ((hi - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF)
                            } else {
                                hi
                            };
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(self.err("잘못된 이스케이프")),
                    }
                    run = self.pos;
                }
                Some(_) => self.pos += 1,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_and_pointer() {
        let src = r#"[null,{"name":"하롤드","list":[{"code":401,"parameters":["a\"bあ"]}]}]"#;
        let root = parse(src).unwrap();
        let n = root.pointer("/1/name").unwrap();
        assert_eq!(n.as_str(), Some("하롤드"));
        assert_eq!(&src[n.start..n.end], "\"하롤드\"");
        let t = root.pointer("/1/list/0/parameters/0").unwrap();
        assert_eq!(t.as_str(), Some("a\"bあ"));
        assert_eq!(root.pointer("/1/list/0/code").unwrap().as_i64(), Some(401));
    }
}
