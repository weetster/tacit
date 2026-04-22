"""Parser for canonical text. Bytes → typed AST.

Two passes:
  1. Tokens → nested s-expression items (SList / SAtom).
  2. Items → typed AST, validating tag, arity, and child shape.

Rejections surface as ParseError (structural) or LexError (lexical).
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Union

from . import ast as A
from .lex import (
    IntTok,
    LParen,
    LexError,
    RParen,
    StrTok,
    SymTok,
    Token,
    lex,
)


class ParseError(Exception):
    pass


@dataclass(frozen=True, slots=True)
class SAtom:
    tok: Token


@dataclass(frozen=True, slots=True)
class SList:
    items: tuple["SItem", ...]


SItem = Union[SAtom, SList]


def parse(data: bytes) -> A.Node:
    tokens = lex(data)
    item, pos = _read_item(tokens, 0)
    if pos != len(tokens):
        raise ParseError(f"trailing tokens after position {pos}")
    return _build(item)


def _read_item(tokens: list[Token], pos: int) -> tuple[SItem, int]:
    if pos >= len(tokens):
        raise ParseError("unexpected end of input")
    t = tokens[pos]
    if isinstance(t, LParen):
        pos += 1
        items: list[SItem] = []
        while pos < len(tokens) and not isinstance(tokens[pos], RParen):
            child, pos = _read_item(tokens, pos)
            items.append(child)
        if pos >= len(tokens):
            raise ParseError("unclosed '('")
        return SList(tuple(items)), pos + 1
    if isinstance(t, RParen):
        raise ParseError(f"unexpected ')' at token {pos}")
    return SAtom(t), pos + 1


def _build(item: SItem) -> A.Node:
    if isinstance(item, SAtom):
        tok = item.tok
        if isinstance(tok, SymTok):
            if tok.name == "pat-wild":
                return A.PatWild()
            if tok.name == "pat-var":
                return A.PatVar()
            raise ParseError(f"bare symbol {tok.name!r} is not an expression")
        raise ParseError(f"bare atom {tok!r} is not an expression")

    if not item.items:
        raise ParseError("empty list")
    head = item.items[0]
    if not (isinstance(head, SAtom) and isinstance(head.tok, SymTok)):
        raise ParseError("list head must be a tag symbol")
    tag = head.tok.name
    args = item.items[1:]
    return _build_form(tag, args)


def _build_form(tag: str, args: tuple[SItem, ...]) -> A.Node:
    match tag:
        case "lam":
            _check_arity(tag, args, 1)
            return A.Lam(_build(args[0]))
        case "app":
            _check_arity(tag, args, 2)
            return A.App(_build(args[0]), _build(args[1]))
        case "let":
            _check_arity(tag, args, 2)
            return A.Let(_build(args[0]), _build(args[1]))
        case "rec":
            if len(args) < 2:
                raise ParseError(
                    "rec requires at least 1 binding + body (ADR 0011)"
                )
            bindings = tuple(_build(a) for a in args[:-1])
            return A.Rec(bindings, _build(args[-1]))
        case "module":
            if len(args) < 1:
                raise ParseError(
                    "module requires at least 1 binding (ADR 0011)"
                )
            return A.Module(tuple(_build(a) for a in args))
        case "if":
            _check_arity(tag, args, 3)
            return A.If_(_build(args[0]), _build(args[1]), _build(args[2]))
        case "match":
            if len(args) < 2:
                raise ParseError(
                    "match requires scrutinee + at least 1 arm (ADR 0011)"
                )
            scrutinee = _build(args[0])
            arms = []
            for a in args[1:]:
                arm = _build(a)
                if not isinstance(arm, A.Arm):
                    raise ParseError("match child must be an arm")
                arms.append(arm)
            return A.Match(scrutinee, tuple(arms))
        case "arm":
            _check_arity(tag, args, 2)
            return A.Arm(_build(args[0]), _build(args[1]))
        case "record":
            if len(args) % 2 != 0:
                raise ParseError(
                    "record requires an even number of children (sym/val pairs)"
                )
            fields: list[tuple[str, A.Node]] = []
            for k_item, v_item in zip(args[0::2], args[1::2]):
                fields.append((_bare_sym(k_item, "record field"), _build(v_item)))
            return A.Record(tuple(fields))
        case "proj":
            _check_arity(tag, args, 2)
            return A.Proj(_build(args[0]), _bare_sym(args[1], "proj field"))
        case "ctor":
            if len(args) < 1:
                raise ParseError("ctor requires a name symbol")
            name = _bare_sym(args[0], "ctor name")
            sub = tuple(_build(a) for a in args[1:])
            return A.Ctor(name, sub)
        case "ann":
            _check_arity(tag, args, 2)
            return A.Ann(_build(args[0]), _build(args[1]))
        case "var":
            _check_arity(tag, args, 1)
            n_text = _bare_int(args[0], "var index")
            n = int(n_text)
            if n < 0:
                raise ParseError("var index must be >= 0")
            return A.Var(n)
        case "int":
            _check_arity(tag, args, 1)
            text = _bare_int(args[0], "int literal")
            return A.Int_(int(text))
        case "str":
            _check_arity(tag, args, 1)
            return A.Str_(_bare_str(args[0], "str literal"))
        case "sym":
            _check_arity(tag, args, 1)
            return A.Sym(_bare_sym(args[0], "sym atom"))
        case "hole":
            _check_arity(tag, args, 2)
            diag = _bare_sym(args[0], "hole diag-id")
            payload = _build(args[1])
            if not isinstance(payload, A.Str_):
                raise ParseError("hole payload must be a (str ...) node")
            return A.Hole(diag, payload)
        case "pat-ctor":
            if len(args) < 1:
                raise ParseError("pat-ctor requires a name symbol")
            name = _bare_sym(args[0], "pat-ctor name")
            sub = tuple(_build(a) for a in args[1:])
            return A.PatCtor(name, sub)
        case _:
            raise ParseError(f"unknown tag {tag!r}")


def _check_arity(tag: str, args: tuple[SItem, ...], expected: int) -> None:
    if len(args) != expected:
        raise ParseError(
            f"{tag} expects arity {expected}, got {len(args)}"
        )


def _bare_sym(item: SItem, what: str) -> str:
    if isinstance(item, SAtom) and isinstance(item.tok, SymTok):
        return item.tok.name
    raise ParseError(f"{what} must be a bare symbol")


def _bare_int(item: SItem, what: str) -> str:
    if isinstance(item, SAtom) and isinstance(item.tok, IntTok):
        return item.tok.text
    raise ParseError(f"{what} must be a decimal integer atom")


def _bare_str(item: SItem, what: str) -> str:
    if isinstance(item, SAtom) and isinstance(item.tok, StrTok):
        return item.tok.value
    raise ParseError(f"{what} must be a string atom")


__all__ = ["parse", "ParseError", "LexError"]
