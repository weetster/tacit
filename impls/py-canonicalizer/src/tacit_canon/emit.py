"""Emitter: typed AST → canonical UTF-8 bytes.

Enforces:
  - Surface form: s-expressions, exactly one ASCII space between children,
    no whitespace inside parens (canonical-text-format.md § 1, 3).
  - ADR 0008: record fields sorted ascending by UTF-8 byte sequence
    of the field symbol.
  - ADR 0010 I1: `-0` (i.e. AST value 0) emits as `0`.
  - ADR 0010 I2: arbitrary-precision integers (Python int is unbounded).
  - ADR 0010 S1-S4: named escapes preferred, non-ASCII and unnamed
    controls emit as `\\u{HEX}` in shortest lowercase form, with
    `\\u{0}` the sole "leading zero" allowed (NUL).
"""

from __future__ import annotations

from . import ast as A


def emit(node: A.Node) -> bytes:
    return _emit(node).encode("utf-8")


def _emit(node: A.Node) -> str:
    match node:
        case A.Lam(body):
            return f"(lam {_emit(body)})"
        case A.App(fn, arg):
            return f"(app {_emit(fn)} {_emit(arg)})"
        case A.Let(rhs, body):
            return f"(let {_emit(rhs)} {_emit(body)})"
        case A.Rec(bindings, body):
            inner = " ".join(_emit(b) for b in bindings)
            return f"(rec {inner} {_emit(body)})"
        case A.Module(bindings):
            inner = " ".join(_emit(b) for b in bindings)
            return f"(module {inner})"
        case A.If_(cond, then, else_):
            return f"(if {_emit(cond)} {_emit(then)} {_emit(else_)})"
        case A.Match(scrutinee, arms):
            inner = " ".join(_emit(a) for a in arms)
            return f"(match {_emit(scrutinee)} {inner})"
        case A.Arm(pattern, body):
            return f"(arm {_emit(pattern)} {_emit(body)})"
        case A.Record(fields):
            if not fields:
                return "(record)"
            ordered = sorted(fields, key=lambda kv: kv[0].encode("utf-8"))
            parts: list[str] = []
            for k, v in ordered:
                parts.append(k)
                parts.append(_emit(v))
            return "(record " + " ".join(parts) + ")"
        case A.Proj(record, field):
            return f"(proj {_emit(record)} {field})"
        case A.Ctor(name, args):
            if not args:
                return f"(ctor {name})"
            inner = " ".join(_emit(a) for a in args)
            return f"(ctor {name} {inner})"
        case A.Ann(expr, type_):
            return f"(ann {_emit(expr)} {_emit(type_)})"
        case A.Var(index):
            return f"(var {index})"
        case A.Int_(value):
            # ADR 0010 I1: any AST-level "negative zero" renders as 0.
            # Python already normalizes int(-0) == 0, so just stringify.
            return f"(int {value})"
        case A.Str_(value):
            return f"(str {_emit_string(value)})"
        case A.Sym(name):
            return f"(sym {name})"
        case A.Hole(diag_id, payload):
            return f"(hole {diag_id} {_emit(payload)})"
        case A.PatWild():
            return "pat-wild"
        case A.PatVar():
            return "pat-var"
        case A.PatCtor(name, sub):
            if not sub:
                return f"(pat-ctor {name})"
            inner = " ".join(_emit(s) for s in sub)
            return f"(pat-ctor {name} {inner})"
        case _:  # pragma: no cover - exhaustive by construction
            raise TypeError(f"cannot emit {node!r}")


_NAMED_ESCAPES = {
    0x22: '\\"',
    0x5C: "\\\\",
    0x09: "\\t",
    0x0A: "\\n",
    0x0D: "\\r",
}


def _emit_string(s: str) -> str:
    """Return the canonical string literal (including surrounding quotes)."""
    parts: list[str] = ['"']
    for ch in s:
        cp = ord(ch)
        named = _NAMED_ESCAPES.get(cp)
        if named is not None:
            parts.append(named)
        elif 0x20 <= cp <= 0x7E:
            parts.append(ch)
        else:
            # ADR 0010 S3: shortest lowercase hex, no leading zeros
            # (`\u{0}` for NUL is the one permitted single-zero form).
            parts.append(f"\\u{{{cp:x}}}")
    parts.append('"')
    return "".join(parts)
