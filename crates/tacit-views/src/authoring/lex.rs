//! Lexer for the authoring view (BPE-compact surface syntax).
//!
//! Tokens include keywords, identifiers, integer/string literals, and
//! single-character punctuation. Whitespace (space, tab, newline) is skipped.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    UnexpectedByte(u8, usize),
    UnterminatedString(usize),
    BadEscape(String),
    BadCodepoint(u32),
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexError::UnexpectedByte(b, i) => write!(f, "unexpected byte 0x{:02x} at {}", b, i),
            LexError::UnterminatedString(i) => write!(f, "unterminated string at {}", i),
            LexError::BadEscape(s) => write!(f, "bad escape: {}", s),
            LexError::BadCodepoint(cp) => write!(f, "invalid codepoint U+{:X}", cp),
        }
    }
}

impl std::error::Error for LexError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    // Structural keywords
    Lambda,
    Let,
    Rec,
    In,
    If,
    Then,
    Else,
    Match,
    With,
    // Value-carrying
    Ident(String),
    Int(String),
    Str(String),
    // Punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    Dot,
    Comma,
    Colon,
    Semicolon,
    Eq,
    FatArrow,
    Pipe,
    At,
    Underscore,
}

pub fn lex(input: &[u8]) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let mut i = 0;
    let n = input.len();
    while i < n {
        let b = input[i];
        match b {
            // Whitespace
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            // Punctuation
            b'(' => { tokens.push(Token::LParen); i += 1; }
            b')' => { tokens.push(Token::RParen); i += 1; }
            b'{' => { tokens.push(Token::LBrace); i += 1; }
            b'}' => { tokens.push(Token::RBrace); i += 1; }
            b'.' => { tokens.push(Token::Dot); i += 1; }
            b',' => { tokens.push(Token::Comma); i += 1; }
            b':' => { tokens.push(Token::Colon); i += 1; }
            b';' => { tokens.push(Token::Semicolon); i += 1; }
            b'|' => { tokens.push(Token::Pipe); i += 1; }
            b'@' => { tokens.push(Token::At); i += 1; }
            b'=' => {
                if i + 1 < n && input[i + 1] == b'>' {
                    tokens.push(Token::FatArrow);
                    i += 2;
                } else {
                    tokens.push(Token::Eq);
                    i += 1;
                }
            }
            // String literal
            b'"' => {
                let (s, ni) = lex_string(input, i)?;
                tokens.push(Token::Str(s));
                i = ni;
            }
            // Negative integer
            b'-' if i + 1 < n && input[i + 1].is_ascii_digit() => {
                let (s, ni) = lex_int(input, i);
                tokens.push(Token::Int(s));
                i = ni;
            }
            // Positive integer
            b'0'..=b'9' => {
                let (s, ni) = lex_int(input, i);
                tokens.push(Token::Int(s));
                i = ni;
            }
            // Identifier or keyword or underscore
            b'_' => {
                // If followed by ident chars, it's an identifier; alone it's Underscore.
                if i + 1 < n && is_ident_cont(input[i + 1]) {
                    let (name, ni) = lex_ident(input, i);
                    tokens.push(Token::Ident(name));
                    i = ni;
                } else {
                    tokens.push(Token::Underscore);
                    i += 1;
                }
            }
            b if is_ident_start(b) => {
                let (name, ni) = lex_ident(input, i);
                tokens.push(keyword_or_ident(name));
                i = ni;
            }
            _ => return Err(LexError::UnexpectedByte(b, i)),
        }
    }
    Ok(tokens)
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic()
}

fn is_ident_cont(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn lex_ident(input: &[u8], start: usize) -> (String, usize) {
    let mut i = start;
    while i < input.len() && is_ident_cont(input[i]) {
        i += 1;
    }
    (String::from_utf8_lossy(&input[start..i]).into_owned(), i)
}

fn lex_int(input: &[u8], start: usize) -> (String, usize) {
    let mut i = start;
    if i < input.len() && input[i] == b'-' {
        i += 1;
    }
    while i < input.len() && input[i].is_ascii_digit() {
        i += 1;
    }
    (String::from_utf8_lossy(&input[start..i]).into_owned(), i)
}

fn keyword_or_ident(name: String) -> Token {
    match name.as_str() {
        "lambda" => Token::Lambda,
        "let"    => Token::Let,
        "rec"    => Token::Rec,
        "in"     => Token::In,
        "if"     => Token::If,
        "then"   => Token::Then,
        "else"   => Token::Else,
        "match"  => Token::Match,
        "with"   => Token::With,
        _        => Token::Ident(name),
    }
}

fn lex_string(input: &[u8], start: usize) -> Result<(String, usize), LexError> {
    debug_assert_eq!(input[start], b'"');
    let mut i = start + 1;
    let n = input.len();
    let mut out = String::new();
    loop {
        if i >= n {
            return Err(LexError::UnterminatedString(start));
        }
        let b = input[i];
        match b {
            b'"' => return Ok((out, i + 1)),
            b'\\' => {
                let (ch, ni) = lex_escape(input, i)?;
                out.push(ch);
                i = ni;
            }
            0x00..=0x1F | 0x7F => {
                return Err(LexError::UnexpectedByte(b, i));
            }
            0x80..=0xFF => {
                let (ch, ni) = lex_utf8(input, i)
                    .map_err(|e| LexError::BadEscape(e.to_string()))?;
                out.push(ch);
                i = ni;
            }
            _ => {
                out.push(b as char);
                i += 1;
            }
        }
    }
}

fn lex_escape(input: &[u8], i: usize) -> Result<(char, usize), LexError> {
    if i + 1 >= input.len() {
        return Err(LexError::BadEscape("dangling backslash".to_string()));
    }
    match input[i + 1] {
        b'"'  => Ok(('"',  i + 2)),
        b'\\' => Ok(('\\', i + 2)),
        b'n'  => Ok(('\n', i + 2)),
        b't'  => Ok(('\t', i + 2)),
        b'r'  => Ok(('\r', i + 2)),
        b'u'  => lex_unicode_escape(input, i),
        other => Err(LexError::BadEscape(format!("\\{}", other as char))),
    }
}

fn lex_unicode_escape(input: &[u8], i: usize) -> Result<(char, usize), LexError> {
    let n = input.len();
    if i + 2 >= n || input[i + 2] != b'{' {
        return Err(LexError::BadEscape("expected '{' after \\u".to_string()));
    }
    let hex_start = i + 3;
    let mut j = hex_start;
    while j < n && input[j] != b'}' {
        j += 1;
    }
    if j >= n {
        return Err(LexError::BadEscape("unterminated \\u{".to_string()));
    }
    let hex = &input[hex_start..j];
    if hex.is_empty() || hex.len() > 6 {
        return Err(LexError::BadEscape("\\u{} requires 1-6 hex digits".to_string()));
    }
    let text = std::str::from_utf8(hex)
        .map_err(|_| LexError::BadEscape("non-ascii in \\u{}".to_string()))?;
    let cp = u32::from_str_radix(text, 16)
        .map_err(|_| LexError::BadEscape(format!("bad hex {:?}", text)))?;
    if (0xD800..=0xDFFF).contains(&cp) || cp > 0x10FFFF {
        return Err(LexError::BadCodepoint(cp));
    }
    let ch = char::from_u32(cp).ok_or(LexError::BadCodepoint(cp))?;
    Ok((ch, j + 1))
}

fn lex_utf8(input: &[u8], i: usize) -> Result<(char, usize), LexError> {
    let b = input[i];
    let width = if b >> 5 == 0b110 { 2 }
        else if b >> 4 == 0b1110 { 3 }
        else if b >> 3 == 0b11110 { 4 }
        else { return Err(LexError::UnexpectedByte(b, i)); };
    if i + width > input.len() {
        return Err(LexError::UnexpectedByte(b, i));
    }
    let s = std::str::from_utf8(&input[i..i + width])
        .map_err(|_| LexError::UnexpectedByte(b, i))?;
    let ch = s.chars().next().unwrap();
    let cp = ch as u32;
    if (0xD800..=0xDFFF).contains(&cp) {
        return Err(LexError::BadCodepoint(cp));
    }
    Ok((ch, i + width))
}
