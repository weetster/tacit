"""Typed AST nodes for the Tacit-Lite canonical text format.

Kinds and arities match canonical-text-format.md § 2. Variable-arity
minimums (ADR 0011) are enforced at AST construction time in parse.py.
Record field ordering (ADR 0008) is applied at emit time.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Union


@dataclass(frozen=True, slots=True)
class Lam:
    body: "Node"


@dataclass(frozen=True, slots=True)
class App:
    fn: "Node"
    arg: "Node"


@dataclass(frozen=True, slots=True)
class Let:
    rhs: "Node"
    body: "Node"


@dataclass(frozen=True, slots=True)
class Rec:
    bindings: tuple["Node", ...]
    body: "Node"


@dataclass(frozen=True, slots=True)
class Module:
    bindings: tuple["Node", ...]


@dataclass(frozen=True, slots=True)
class If_:
    cond: "Node"
    then: "Node"
    else_: "Node"


@dataclass(frozen=True, slots=True)
class Match:
    scrutinee: "Node"
    arms: tuple["Arm", ...]


@dataclass(frozen=True, slots=True)
class Arm:
    pattern: "Node"
    body: "Node"


@dataclass(frozen=True, slots=True)
class Record:
    fields: tuple[tuple[str, "Node"], ...]


@dataclass(frozen=True, slots=True)
class Proj:
    record: "Node"
    field: str


@dataclass(frozen=True, slots=True)
class Ctor:
    name: str
    args: tuple["Node", ...]


@dataclass(frozen=True, slots=True)
class Ann:
    expr: "Node"
    type_: "Node"


@dataclass(frozen=True, slots=True)
class Var:
    index: int


@dataclass(frozen=True, slots=True)
class Int_:
    value: int


@dataclass(frozen=True, slots=True)
class Str_:
    value: str


@dataclass(frozen=True, slots=True)
class Sym:
    name: str


@dataclass(frozen=True, slots=True)
class Hole:
    diag_id: str
    payload: Str_


@dataclass(frozen=True, slots=True)
class PatWild:
    pass


@dataclass(frozen=True, slots=True)
class PatVar:
    pass


@dataclass(frozen=True, slots=True)
class PatCtor:
    name: str
    sub_patterns: tuple["Node", ...]


@dataclass(frozen=True, slots=True)
class PatInt:
    value: int


@dataclass(frozen=True, slots=True)
class FnTy:
    arg: "Node"
    ret: "Node"
    eff: "Node"


@dataclass(frozen=True, slots=True)
class TyVar:
    index: int


@dataclass(frozen=True, slots=True)
class Forall:
    ty_count: int
    eff_count: int
    body: "Node"


@dataclass(frozen=True, slots=True)
class EffSet:
    atoms: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class EffVar:
    index: int


Node = Union[
    Lam, App, Let, Rec, Module, If_, Match, Arm,
    Record, Proj, Ctor, Ann,
    Var, Int_, Str_, Sym, Hole,
    PatWild, PatVar, PatCtor,
    PatInt, FnTy, TyVar, Forall, EffSet, EffVar,
]
