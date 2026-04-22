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
    Lam { body: Box<Node> },
    App { fn_: Box<Node>, arg: Box<Node> },
    Let { rhs: Box<Node>, body: Box<Node> },
    Rec { bindings: Vec<Node>, body: Box<Node> },
    Module { bindings: Vec<Node> },
    If { cond: Box<Node>, then: Box<Node>, else_: Box<Node> },
    Match { scrutinee: Box<Node>, arms: Vec<Node> },
    Arm { pattern: Box<Node>, body: Box<Node> },
    Record { fields: Vec<(String, Node)> },
    Proj { record: Box<Node>, field: String },
    Ctor { name: String, args: Vec<Node> },
    Ann { expr: Box<Node>, type_: Box<Node> },
    Var { index: u64 },
    Int { value: String },
    Str { value: String },
    Sym { name: String },
    Hole { diag_id: String, payload: Box<Node> },
    PatWild,
    PatVar,
    PatCtor { name: String, sub_patterns: Vec<Node> },
}

impl Node {
    /// Construct an `Int` node, normalizing `-0` → `0` per ADR 0010 I1.
    /// Input must already be a valid canonical decimal (no leading zeros,
    /// optional leading `-`); callers are parse.rs and tests.
    pub fn int_from_decimal(decimal: &str) -> Node {
        let normalized = if decimal == "-0" { "0".to_string() } else { decimal.to_string() };
        Node::Int { value: normalized }
    }
}
