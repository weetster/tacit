//! Lexer for the canonical text format.
//!
//! Operates on UTF-8 bytes. Tokens:
//!   - LParen `(` / RParen `)`
//!   - IntTok: optional leading '-', then 0 or [1-9][0-9]*
//!   - SymTok: [A-Za-z_][A-Za-z0-9_-]*
//!   - StrTok: double-quoted; \", \\, \n, \t, \r, \u{HEX} supported.
//!
//! Rules enforced here:
//!   - Raw control bytes (0x00-0x1f, 0x7f) inside a string are errors.
//!   - \u{HEX}: 1-6 lowercase hex digits. Surrogates (U+D800..U+DFFF) and
//!     values > U+10FFFF are hard errors (ADR 0012).
//!   - Integer leading zeros forbidden (other than the single digit `0`).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError(pub String);

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LexError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    LParen,
    RParen,
    Int(String),
    Str(String),
    Sym(String),
}

fn is_sym_start(b: u8) -> bool {
    (0x41..=0x5A).contains(&b) || (0x61..=0x7A).contains(&b) || b == 0x5F
}

fn is_sym_cont(b: u8) -> bool {
    is_sym_start(b) || (0x30..=0x39).contains(&b) || b == 0x2D
}

fn is_hex_lower(b: u8) -> bool {
    (0x30..=0x39).contains(&b) || (0x61..=0x66).contains(&b)
}

pub fn lex(data: &[u8]) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let mut i = 0usize;
    let n = data.len();
    while i < n {
        let b = data[i];
        match b {
            0x28 => {
                tokens.push(Token::LParen);
                i += 1;
            }
            0x29 => {
                tokens.push(Token::RParen);
                i += 1;
            }
            0x20 => {
                i += 1;
            }
            0x22 => {
                let (value, ni) = lex_string(data, i)?;
                tokens.push(Token::Str(value));
                i = ni;
            }
            0x2D | 0x30..=0x39 => {
                let (text, ni) = lex_int(data, i)?;
                tokens.push(Token::Int(text));
                i = ni;
            }
            b if is_sym_start(b) => {
                let (name, ni) = lex_sym(data, i);
                tokens.push(Token::Sym(name));
                i = ni;
            }
            _ => {
                return Err(LexError(format!("unexpected byte 0x{:02x} at offset {}", b, i)));
            }
        }
    }
    Ok(tokens)
}

fn lex_int(data: &[u8], start: usize) -> Result<(String, usize), LexError> {
    let mut i = start;
    if data[i] == 0x2D {
        i += 1;
        if i >= data.len() || !(0x30..=0x39).contains(&data[i]) {
            return Err(LexError(format!("lone '-' at offset {}", start)));
        }
    }
    if data[i] == 0x30 {
        i += 1;
        if i < data.len() && (0x30..=0x39).contains(&data[i]) {
            return Err(LexError(format!("leading zeros in integer at offset {}", start)));
        }
    } else {
        while i < data.len() && (0x30..=0x39).contains(&data[i]) {
            i += 1;
        }
    }
    let text = std::str::from_utf8(&data[start..i])
        .map_err(|e| LexError(format!("non-ascii in integer: {}", e)))?
        .to_string();
    Ok((text, i))
}

fn lex_sym(data: &[u8], start: usize) -> (String, usize) {
    let mut i = start + 1;
    while i < data.len() && is_sym_cont(data[i]) {
        i += 1;
    }
    // Safe: every accepted byte in is_sym_start / is_sym_cont is ASCII.
    let name = std::str::from_utf8(&data[start..i]).unwrap().to_string();
    (name, i)
}

fn lex_string(data: &[u8], start: usize) -> Result<(String, usize), LexError> {
    debug_assert_eq!(data[start], 0x22);
    let mut i = start + 1;
    let n = data.len();
    let mut out = String::new();
    loop {
        if i >= n {
            return Err(LexError("unterminated string literal".to_string()));
        }
        let b = data[i];
        if b == 0x22 {
            return Ok((out, i + 1));
        }
        if b == 0x5C {
            let (ch, ni) = lex_escape(data, i)?;
            out.push(ch);
            i = ni;
            continue;
        }
        if b < 0x20 || b == 0x7F {
            return Err(LexError(format!(
                "raw control byte 0x{:02x} in string at offset {}",
                b, i
            )));
        }
        if b < 0x80 {
            out.push(b as char);
            i += 1;
            continue;
        }
        let (ch, ni) = lex_utf8(data, i)?;
        out.push(ch);
        i = ni;
    }
}

fn lex_escape(data: &[u8], i: usize) -> Result<(char, usize), LexError> {
    let n = data.len();
    if i + 1 >= n {
        return Err(LexError("dangling backslash in string".to_string()));
    }
    let kind = data[i + 1];
    match kind {
        0x22 => Ok(('"', i + 2)),
        0x5C => Ok(('\\', i + 2)),
        0x6E => Ok(('\n', i + 2)),
        0x74 => Ok(('\t', i + 2)),
        0x72 => Ok(('\r', i + 2)),
        0x75 => {
            if i + 2 >= n || data[i + 2] != 0x7B {
                return Err(LexError(format!("expected '{{' after \\u at offset {}", i)));
            }
            let hex_start = i + 3;
            let mut j = hex_start;
            while j < n && data[j] != 0x7D {
                j += 1;
            }
            if j >= n {
                return Err(LexError(format!("unterminated \\u{{ at offset {}", i)));
            }
            let digits = &data[hex_start..j];
            if digits.is_empty() || digits.len() > 6 {
                return Err(LexError(format!("\\u{{HEX}} must be 1-6 hex digits at offset {}", i)));
            }
            for &d in digits {
                if !is_hex_lower(d) {
                    return Err(LexError(format!("\\u{{HEX}} requires lowercase hex at offset {}", i)));
                }
            }
            let text = std::str::from_utf8(digits).unwrap();
            let cp = u32::from_str_radix(text, 16).unwrap();
            if (0xD800..=0xDFFF).contains(&cp) {
                return Err(LexError(format!("surrogate code point U+{:04X} (ADR 0012)", cp)));
            }
            if cp > 0x10FFFF {
                return Err(LexError(format!("code point out of range U+{:X} (ADR 0012)", cp)));
            }
            let ch = char::from_u32(cp).ok_or_else(|| {
                LexError(format!("invalid code point U+{:X}", cp))
            })?;
            Ok((ch, j + 1))
        }
        _ => Err(LexError(format!("unknown escape \\{} at offset {}", kind as char, i))),
    }
}

fn lex_utf8(data: &[u8], i: usize) -> Result<(char, usize), LexError> {
    let b = data[i];
    let width = if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        return Err(LexError(format!("invalid UTF-8 lead byte 0x{:02x} at offset {}", b, i)));
    };
    if i + width > data.len() {
        return Err(LexError(format!("truncated UTF-8 sequence at offset {}", i)));
    }
    let s = std::str::from_utf8(&data[i..i + width])
        .map_err(|e| LexError(format!("invalid UTF-8 at offset {}: {}", i, e)))?;
    let mut chars = s.chars();
    let ch = chars.next().ok_or_else(|| LexError(format!("empty UTF-8 at offset {}", i)))?;
    if chars.next().is_some() {
        return Err(LexError(format!("unexpected multi-scalar UTF-8 at offset {}", i)));
    }
    let cp = ch as u32;
    if (0xD800..=0xDFFF).contains(&cp) {
        return Err(LexError(format!("surrogate via raw UTF-8 U+{:04X}", cp)));
    }
    Ok((ch, i + width))
}
