//! Typed AST nodes for the Tacit-Lite canonical text format.
//!
//! Kinds and arities match canonical-text-format.md § 2. Variable-arity
//! minimums (ADR 0011) are enforced at AST construction time in parse.rs.
//! Record field ordering (ADR 0008) is applied at emit time.
//!
//! Integer literals are stored as already-normalized decimal strings
//! (ADR 0010 I1 `-0` → `0`; I2 arbitrary precision) so this layer never
//! touches bounded integer types.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Lam {
        body: Box<Node>,
    },
    App {
        fn_: Box<Node>,
        arg: Box<Node>,
    },
    Let {
        rhs: Box<Node>,
        body: Box<Node>,
    },
    Rec {
        bindings: Vec<Node>,
        body: Box<Node>,
    },
    Module {
        bindings: Vec<Node>,
    },
    Unit {
        imports: Vec<Node>,
        exports: Vec<Node>,
        defs: Vec<Node>,
    },
    Imports {
        entries: Vec<Node>,
    },
    Import {
        hash: String,
        sig: Box<Node>,
    },
    Exports {
        entries: Vec<Node>,
    },
    Export {
        visibility: String,
        hash: String,
    },
    Defs {
        defs: Vec<Node>,
    },
    Def {
        sig: Box<Node>,
        body: Box<Node>,
    },
    Sig {
        type_: Box<Node>,
        eval_eff: Box<Node>,
    },
    Ref {
        hash: String,
    },
    If {
        cond: Box<Node>,
        then: Box<Node>,
        else_: Box<Node>,
    },
    Match {
        scrutinee: Box<Node>,
        arms: Vec<Node>,
    },
    Arm {
        pattern: Box<Node>,
        body: Box<Node>,
    },
    Record {
        fields: Vec<(String, Node)>,
    },
    Proj {
        record: Box<Node>,
        field: String,
    },
    Ctor {
        name: String,
        args: Vec<Node>,
    },
    Ann {
        expr: Box<Node>,
        type_: Box<Node>,
    },
    Var {
        index: u64,
    },
    Int {
        value: String,
    },
    Str {
        value: String,
    },
    Sym {
        name: String,
    },
    Hole {
        diag_id: String,
        payload: Box<Node>,
    },
    PatWild,
    PatVar,
    PatCtor {
        name: String,
        sub_patterns: Vec<Node>,
    },
    /// `(pat-int N)` — integer literal pattern (ADR 0037).
    PatInt {
        value: String,
    },
    /// `(fn-ty arg ret eff)` — function type (ADR 0034).
    FnTy {
        arg: Box<Node>,
        ret: Box<Node>,
        eff: Box<Node>,
    },
    /// `(ty-var N)` — type variable reference, DeBruijn over `forall` (ADR 0034).
    TyVar {
        index: u64,
    },
    /// `(forall TY-COUNT EFF-COUNT body)` — universal quantification (ADR 0034).
    Forall {
        ty_count: u32,
        eff_count: u32,
        body: Box<Node>,
    },
    /// `(eff-set atom*)` — concrete effect set, atoms sorted (ADR 0035).
    EffSet {
        atoms: Vec<String>,
    },
    /// `(eff-var N)` — effect variable reference, DeBruijn over `forall` (ADR 0036).
    EffVar {
        index: u64,
    },
}

impl Node {
    /// Construct an `Int` node, normalizing `-0` → `0` per ADR 0010 I1.
    /// Input must already be a valid canonical decimal (no leading zeros,
    /// optional leading `-`); callers are parse.rs and tests.
    pub fn int_from_decimal(decimal: &str) -> Node {
        let normalized = if decimal == "-0" {
            "0".to_string()
        } else {
            decimal.to_string()
        };
        Node::Int { value: normalized }
    }
}
