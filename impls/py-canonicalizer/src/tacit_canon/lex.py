"""Lexer for the canonical text format.

Operates on UTF-8 bytes. Tokens:
  - LParen `(` / RParen `)`
  - IntTok: optional leading '-', then 0 or [1-9][0-9]*
  - SymTok: [A-Za-z_][A-Za-z0-9_-]*
  - StrTok: double-quoted; \\\", \\\\, \\n, \\t, \\r, \\u{HEX} supported.

Rules enforced here:
  - Raw control bytes (0x00-0x1f, 0x7f) inside a string are errors.
  - \\u{HEX}: 1-6 lowercase hex digits. Surrogates (U+D800..U+DFFF) and
    values > U+10FFFF are hard errors (ADR 0012).
  - Integer leading zeros forbidden (other than the single digit `0`).
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Union


class LexError(Exception):
    pass


@dataclass(frozen=True, slots=True)
class LParen:
    pass


@dataclass(frozen=True, slots=True)
class RParen:
    pass


@dataclass(frozen=True, slots=True)
class IntTok:
    text: str


@dataclass(frozen=True, slots=True)
class StrTok:
    value: str


@dataclass(frozen=True, slots=True)
class SymTok:
    name: str


Token = Union[LParen, RParen, IntTok, StrTok, SymTok]


_HEX = frozenset(b"0123456789abcdef")


def _is_sym_start(b: int) -> bool:
    return (0x41 <= b <= 0x5A) or (0x61 <= b <= 0x7A) or b == 0x5F


def _is_sym_cont(b: int) -> bool:
    return _is_sym_start(b) or (0x30 <= b <= 0x39) or b == 0x2D


def lex(data: bytes) -> list[Token]:
    tokens: list[Token] = []
    i = 0
    n = len(data)
    while i < n:
        b = data[i]
        if b == 0x28:
            tokens.append(LParen())
            i += 1
        elif b == 0x29:
            tokens.append(RParen())
            i += 1
        elif b == 0x20:
            i += 1
        elif b == 0x22:
            value, i = _lex_string(data, i)
            tokens.append(StrTok(value))
        elif b == 0x2D or (0x30 <= b <= 0x39):
            text, i = _lex_int(data, i)
            tokens.append(IntTok(text))
        elif _is_sym_start(b):
            name, i = _lex_sym(data, i)
            tokens.append(SymTok(name))
        else:
            raise LexError(f"unexpected byte 0x{b:02x} at offset {i}")
    return tokens


def _lex_int(data: bytes, i: int) -> tuple[str, int]:
    start = i
    if data[i] == 0x2D:
        i += 1
        if i >= len(data) or not (0x30 <= data[i] <= 0x39):
            raise LexError(f"lone '-' at offset {start}")
    if data[i] == 0x30:
        i += 1
        if i < len(data) and (0x30 <= data[i] <= 0x39):
            raise LexError(f"leading zeros in integer at offset {start}")
    else:
        while i < len(data) and 0x30 <= data[i] <= 0x39:
            i += 1
    return data[start:i].decode("ascii"), i


def _lex_sym(data: bytes, i: int) -> tuple[str, int]:
    start = i
    i += 1
    while i < len(data) and _is_sym_cont(data[i]):
        i += 1
    return data[start:i].decode("ascii"), i


def _lex_string(data: bytes, i: int) -> tuple[str, int]:
    assert data[i] == 0x22
    i += 1
    n = len(data)
    out: list[str] = []
    while True:
        if i >= n:
            raise LexError("unterminated string literal")
        b = data[i]
        if b == 0x22:
            return "".join(out), i + 1
        if b == 0x5C:
            ch, i = _lex_escape(data, i)
            out.append(ch)
            continue
        if b < 0x20 or b == 0x7F:
            raise LexError(
                f"raw control byte 0x{b:02x} in string at offset {i}"
            )
        if b < 0x80:
            out.append(chr(b))
            i += 1
            continue
        # UTF-8 continuation byte expected; decode one scalar value.
        ch, i = _lex_utf8(data, i)
        out.append(ch)


def _lex_escape(data: bytes, i: int) -> tuple[str, int]:
    # data[i] == 0x5C
    n = len(data)
    if i + 1 >= n:
        raise LexError("dangling backslash in string")
    kind = data[i + 1]
    simple = {0x22: '"', 0x5C: "\\", 0x6E: "\n", 0x74: "\t", 0x72: "\r"}
    if kind in simple:
        return simple[kind], i + 2
    if kind != 0x75:
        raise LexError(f"unknown escape \\{chr(kind)} at offset {i}")
    if i + 2 >= n or data[i + 2] != 0x7B:
        raise LexError(f"expected '{{' after \\u at offset {i}")
    j = i + 3
    hex_start = j
    while j < n and data[j] != 0x7D:
        j += 1
    if j >= n:
        raise LexError(f"unterminated \\u{{ at offset {i}")
    digits = data[hex_start:j]
    if not (1 <= len(digits) <= 6):
        raise LexError(
            f"\\u{{HEX}} must be 1-6 hex digits at offset {i}"
        )
    for d in digits:
        if d not in _HEX:
            raise LexError(
                f"\\u{{HEX}} requires lowercase hex at offset {i}"
            )
    cp = int(digits.decode("ascii"), 16)
    if 0xD800 <= cp <= 0xDFFF:
        raise LexError(
            f"surrogate code point U+{cp:04X} (ADR 0012)"
        )
    if cp > 0x10FFFF:
        raise LexError(
            f"code point out of range U+{cp:X} (ADR 0012)"
        )
    return chr(cp), j + 1


def _lex_utf8(data: bytes, i: int) -> tuple[str, int]:
    b = data[i]
    if (b >> 5) == 0b110:
        width = 2
    elif (b >> 4) == 0b1110:
        width = 3
    elif (b >> 3) == 0b11110:
        width = 4
    else:
        raise LexError(f"invalid UTF-8 lead byte 0x{b:02x} at offset {i}")
    if i + width > len(data):
        raise LexError(f"truncated UTF-8 sequence at offset {i}")
    try:
        ch = data[i : i + width].decode("utf-8")
    except UnicodeDecodeError as e:
        raise LexError(f"invalid UTF-8 at offset {i}: {e}") from None
    if len(ch) != 1:
        raise LexError(f"unexpected multi-scalar UTF-8 at offset {i}")
    cp = ord(ch)
    if 0xD800 <= cp <= 0xDFFF:
        raise LexError(f"surrogate via raw UTF-8 U+{cp:04X}")
    return ch, i + width
