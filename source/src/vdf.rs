//! Ordered parsers: preserve unknown fields and duplicate keys, reject unsupported
//! binary types and malformed input rather than silently rewriting Steam data.
use crate::Result;
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Object(Vec<(String, Value)>),
    Text(String),
    I32(u32),
    Float([u8; 4]),
    Pointer([u8; 4]),
    Color([u8; 4]),
    U64([u8; 8]),
    I64([u8; 8]),
}
pub type Object = Vec<(String, Value)>;
pub fn get<'a>(o: &'a Object, key: &str) -> Option<&'a Value> {
    o.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
}
pub fn string<'a>(o: &'a Object, key: &str) -> Option<&'a str> {
    match get(o, key) {
        Some(Value::Text(v)) => Some(v),
        _ => None,
    }
}
pub fn number(o: &Object, key: &str) -> Option<u32> {
    match get(o, key) {
        Some(Value::I32(v)) => Some(*v),
        _ => None,
    }
}
pub fn object<'a>(o: &'a Object, key: &str) -> Option<&'a Object> {
    match get(o, key) {
        Some(Value::Object(v)) => Some(v),
        _ => None,
    }
}
pub fn set(o: &mut Object, key: &str, value: Value) {
    if let Some((_, v)) = o.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(key)) {
        *v = value;
    } else {
        o.push((key.into(), value));
    }
}
pub fn ensure<'a>(o: &'a mut Object, key: &str) -> Result<&'a mut Object> {
    let n = match o.iter().position(|(k, _)| k.eq_ignore_ascii_case(key)) {
        Some(n) => n,
        None => {
            o.push((key.into(), Value::Object(vec![])));
            o.len() - 1
        }
    };
    match &mut o[n].1 {
        Value::Object(x) => Ok(x),
        _ => Err(format!("Expected object at {key}")),
    }
}
struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.i.checked_add(n).ok_or("VDF overflow")?;
        if end > self.b.len() {
            return Err("Truncated binary VDF".into());
        }
        let r = &self.b[self.i..end];
        self.i = end;
        Ok(r)
    }
    fn text(&mut self) -> Result<String> {
        let len = self.b[self.i..]
            .iter()
            .position(|b| *b == 0)
            .ok_or("Unterminated VDF string")?;
        if len > 1024 * 1024 {
            return Err("Oversized VDF string".into());
        }
        let s = std::str::from_utf8(self.take(len)?)
            .map_err(|_| "Non-UTF-8 Steam data cannot be safely edited")?
            .to_string();
        self.take(1)?;
        Ok(s)
    }
    fn obj(&mut self, depth: usize) -> Result<Object> {
        if depth > 64 {
            return Err("VDF nesting limit".into());
        }
        let mut o = vec![];
        loop {
            let tag = self.take(1)?[0];
            if tag == 8 {
                return Ok(o);
            }
            let key = self.text()?;
            let v = match tag {
                0 => Value::Object(self.obj(depth + 1)?),
                1 => Value::Text(self.text()?),
                2 => Value::I32(u32::from_le_bytes(self.take(4)?.try_into().unwrap())),
                3 => Value::Float(self.take(4)?.try_into().unwrap()),
                4 => Value::Pointer(self.take(4)?.try_into().unwrap()),
                6 => Value::Color(self.take(4)?.try_into().unwrap()),
                7 => Value::U64(self.take(8)?.try_into().unwrap()),
                10 => Value::I64(self.take(8)?.try_into().unwrap()),
                _ => {
                    return Err(format!(
                        "Unsupported binary VDF type {tag}; Steam data left unchanged"
                    ))
                }
            };
            o.push((key, v));
            if o.len() > 100_000 {
                return Err("VDF item limit".into());
            }
        }
    }
}
pub fn parse_binary(b: &[u8]) -> Result<Object> {
    if b.len() > 32 * 1024 * 1024 {
        return Err("Steam file exceeds 32 MiB limit".into());
    }
    let mut r = Reader { b, i: 0 };
    let result = r.obj(0)?;
    if r.i != b.len() {
        return Err("Unexpected trailing binary VDF data".into());
    }
    Ok(result)
}
pub fn binary(o: &Object) -> Result<Vec<u8>> {
    fn emit(o: &Object, b: &mut Vec<u8>) -> Result<()> {
        for (k, v) in o {
            if k.contains('\0') {
                return Err("NUL in VDF key".into());
            }
            let tag = match v {
                Value::Object(_) => 0,
                Value::Text(_) => 1,
                Value::I32(_) => 2,
                Value::Float(_) => 3,
                Value::Pointer(_) => 4,
                Value::Color(_) => 6,
                Value::U64(_) => 7,
                Value::I64(_) => 10,
            };
            b.push(tag);
            b.extend(k.as_bytes());
            b.push(0);
            match v {
                Value::Object(x) => emit(x, b)?,
                Value::Text(x) => {
                    if x.contains('\0') {
                        return Err("NUL in VDF value".into());
                    }
                    b.extend(x.as_bytes());
                    b.push(0);
                }
                Value::I32(x) => b.extend(x.to_le_bytes()),
                Value::Float(x) | Value::Pointer(x) | Value::Color(x) => b.extend(x),
                Value::U64(x) | Value::I64(x) => b.extend(x),
            }
        }
        b.push(8);
        Ok(())
    }
    let mut b = vec![];
    emit(o, &mut b)?;
    Ok(b)
}
#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    Open,
    Close,
}
fn tokens(text: &str) -> Result<Vec<Token>> {
    let mut it = text.chars().peekable();
    let mut t = vec![];
    while let Some(c) = it.next() {
        match c {
            c if c.is_whitespace() => (),
            '/' => {
                if it.next() != Some('/') {
                    return Err("Unsupported VDF slash syntax".into());
                }
                for c in it.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '{' => t.push(Token::Open),
            '}' => t.push(Token::Close),
            '"' => {
                let mut word = String::new();
                let mut done = false;
                while let Some(c) = it.next() {
                    if c == '"' {
                        done = true;
                        break;
                    }
                    if c == '\\' {
                        match it.next() {
                            Some('n') => word.push('\n'),
                            Some('t') => word.push('\t'),
                            Some('r') => word.push('\r'),
                            Some('"') => word.push('"'),
                            Some('\\') => word.push('\\'),
                            Some(x) => {
                                word.push('\\');
                                word.push(x);
                            }
                            None => return Err("Incomplete VDF escape".into()),
                        }
                    } else {
                        word.push(c);
                    }
                }
                if !done {
                    return Err("Unclosed VDF string".into());
                }
                t.push(Token::Word(word));
            }
            c => {
                let mut s = c.to_string();
                while let Some(c) = it.peek() {
                    if c.is_whitespace() || *c == '{' || *c == '}' {
                        break;
                    }
                    s.push(it.next().unwrap());
                }
                if s.starts_with('[') {
                    return Err("Conditional VDF requires manual editing".into());
                }
                t.push(Token::Word(s));
            }
        }
    }
    Ok(t)
}
pub fn parse_text(s: &str) -> Result<Object> {
    if s.len() > 32 * 1024 * 1024 {
        return Err("VDF too large".into());
    }
    let t = tokens(s)?;
    fn parse(t: &[Token], i: &mut usize, nested: bool, depth: usize) -> Result<Object> {
        if depth > 64 {
            return Err("VDF nesting limit".into());
        }
        let mut o = vec![];
        while *i < t.len() {
            if t[*i] == Token::Close {
                if !nested {
                    return Err("Unexpected closing brace".into());
                }
                *i += 1;
                return Ok(o);
            }
            let k = match &t[*i] {
                Token::Word(x) => x.clone(),
                _ => return Err("Expected VDF key".into()),
            };
            *i += 1;
            let v = match t.get(*i) {
                Some(Token::Word(x)) => {
                    *i += 1;
                    Value::Text(x.clone())
                }
                Some(Token::Open) => {
                    *i += 1;
                    Value::Object(parse(t, i, true, depth + 1)?)
                }
                _ => return Err("Expected VDF value".into()),
            };
            o.push((k, v));
        }
        if nested {
            Err("Unclosed VDF object".into())
        } else {
            Ok(o)
        }
    }
    parse(&t, &mut 0, false, 0)
}
pub fn text(o: &Object) -> Result<String> {
    fn q(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\t', "\\t")
            .replace('\r', "\\r")
    }
    fn emit(o: &Object, d: usize, s: &mut String) -> Result<()> {
        for (k, v) in o {
            let pad = "\t".repeat(d);
            match v {
                Value::Text(x) => s.push_str(&format!("{pad}\"{}\"\t\"{}\"\n", q(k), q(x))),
                Value::Object(x) => {
                    s.push_str(&format!("{pad}\"{}\"\n{pad}{{\n", q(k)));
                    emit(x, d + 1, s)?;
                    s.push_str(&format!("{pad}}}\n"));
                }
                _ => return Err("Binary field in text VDF".into()),
            }
        }
        Ok(())
    }
    let mut s = String::new();
    emit(o, 0, &mut s)?;
    Ok(s)
}
