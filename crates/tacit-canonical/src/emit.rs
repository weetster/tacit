//! Emitter: typed AST → canonical UTF-8 bytes.
//!
//! Enforces:
//!   - Surface form: s-expressions, exactly one ASCII space between children,
//!     no whitespace inside parens (canonical-text-format.md § 1, 3).
//!   - ADR 0008: record fields sorted ascending by UTF-8 byte sequence
//!     of the field symbol.
//!   - ADR 0010 I1: `-0` normalizes to `0` (done at AST construction in ast.rs).
//!   - ADR 0010 I2: arbitrary-precision integers (stored as decimal strings).
//!   - ADR 0010 S1-S4: named escapes preferred, non-ASCII and unnamed
//!     controls emit as `\u{HEX}` in shortest lowercase form, with
//!     `\u{0}` the sole "leading zero" allowed (NUL).

use crate::ast::Node;

pub fn emit(node: &Node) -> Vec<u8> {
    let mut s = String::new();
    emit_into(node, &mut s);
    s.into_bytes()
}

fn emit_into(node: &Node, out: &mut String) {
    match node {
        Node::Lam { body } => {
            out.push_str("(lam ");
            emit_into(body, out);
            out.push(')');
        }
        Node::App { fn_, arg } => {
            out.push_str("(app ");
            emit_into(fn_, out);
            out.push(' ');
            emit_into(arg, out);
            out.push(')');
        }
        Node::Let { rhs, body } => {
            out.push_str("(let ");
            emit_into(rhs, out);
            out.push(' ');
            emit_into(body, out);
            out.push(')');
        }
        Node::Rec { bindings, body } => {
            out.push_str("(rec");
            for b in bindings {
                out.push(' ');
                emit_into(b, out);
            }
            out.push(' ');
            emit_into(body, out);
            out.push(')');
        }
        Node::Module { bindings } => {
            out.push_str("(module");
            for b in bindings {
                out.push(' ');
                emit_into(b, out);
            }
            out.push(')');
        }
        Node::Unit {
            imports,
            exports,
            defs,
        } => {
            out.push_str("(unit ");
            emit_imports(imports, out);
            out.push(' ');
            emit_exports(exports, out);
            out.push(' ');
            emit_defs(defs, out);
            out.push(')');
        }
        Node::Imports { entries } => emit_imports(entries, out),
        Node::Import { hash, sig } => emit_import(hash, sig, out),
        Node::HostImport {
            capability,
            operation,
            sig,
        } => emit_host_import(capability, operation, sig, out),
        Node::Exports { entries } => emit_exports(entries, out),
        Node::Export { visibility, hash } => emit_export(visibility, hash, out),
        Node::Defs { defs } => emit_defs(defs, out),
        Node::Def { sig, body } => emit_def(sig, body, out),
        Node::State { name, type_ } => {
            out.push_str("(state ");
            out.push_str(name);
            out.push(' ');
            emit_into(type_, out);
            out.push(')');
        }
        Node::Sig { type_, eval_eff } => {
            out.push_str("(sig ");
            emit_into(type_, out);
            out.push(' ');
            emit_into(eval_eff, out);
            out.push(')');
        }
        Node::Ref { hash } => {
            out.push_str("(ref ");
            emit_string(hash, out);
            out.push(')');
        }
        Node::If { cond, then, else_ } => {
            out.push_str("(if ");
            emit_into(cond, out);
            out.push(' ');
            emit_into(then, out);
            out.push(' ');
            emit_into(else_, out);
            out.push(')');
        }
        Node::Match { scrutinee, arms } => {
            out.push_str("(match ");
            emit_into(scrutinee, out);
            for a in arms {
                out.push(' ');
                emit_into(a, out);
            }
            out.push(')');
        }
        Node::Arm { pattern, body } => {
            out.push_str("(arm ");
            emit_into(pattern, out);
            out.push(' ');
            emit_into(body, out);
            out.push(')');
        }
        Node::Record { fields } => {
            if fields.is_empty() {
                out.push_str("(record)");
                return;
            }
            // ADR 0008: sort ascending by UTF-8 bytes of the field symbol.
            let mut ordered: Vec<&(String, Node)> = fields.iter().collect();
            ordered.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            out.push_str("(record");
            for (k, v) in ordered {
                out.push(' ');
                out.push_str(k);
                out.push(' ');
                emit_into(v, out);
            }
            out.push(')');
        }
        Node::Proj { record, field } => {
            out.push_str("(proj ");
            emit_into(record, out);
            out.push(' ');
            out.push_str(field);
            out.push(')');
        }
        Node::Ctor { name, args } => {
            out.push_str("(ctor ");
            out.push_str(name);
            for a in args {
                out.push(' ');
                emit_into(a, out);
            }
            out.push(')');
        }
        Node::Ann { expr, type_ } => {
            out.push_str("(ann ");
            emit_into(expr, out);
            out.push(' ');
            emit_into(type_, out);
            out.push(')');
        }
        Node::Var { index } => {
            out.push_str("(var ");
            out.push_str(&index.to_string());
            out.push(')');
        }
        Node::Int { value } => {
            out.push_str("(int ");
            out.push_str(value);
            out.push(')');
        }
        Node::Str { value } => {
            out.push_str("(str ");
            emit_string(value, out);
            out.push(')');
        }
        Node::Sym { name } => {
            out.push_str("(sym ");
            out.push_str(name);
            out.push(')');
        }
        Node::Hole { diag_id, payload } => {
            out.push_str("(hole ");
            out.push_str(diag_id);
            out.push(' ');
            emit_into(payload, out);
            out.push(')');
        }
        Node::PatWild => out.push_str("pat-wild"),
        Node::PatVar => out.push_str("pat-var"),
        Node::PatCtor { name, sub_patterns } => {
            out.push_str("(pat-ctor ");
            out.push_str(name);
            for s in sub_patterns {
                out.push(' ');
                emit_into(s, out);
            }
            out.push(')');
        }
        Node::PatInt { value } => {
            out.push_str("(pat-int ");
            out.push_str(value);
            out.push(')');
        }
        Node::FnTy { arg, ret, eff } => {
            out.push_str("(fn-ty ");
            emit_into(arg, out);
            out.push(' ');
            emit_into(ret, out);
            out.push(' ');
            emit_into(eff, out);
            out.push(')');
        }
        Node::TyVar { index } => {
            out.push_str("(ty-var ");
            out.push_str(&index.to_string());
            out.push(')');
        }
        Node::Forall {
            ty_count,
            eff_count,
            body,
        } => {
            out.push_str("(forall ");
            out.push_str(&ty_count.to_string());
            out.push(' ');
            out.push_str(&eff_count.to_string());
            out.push(' ');
            emit_into(body, out);
            out.push(')');
        }
        Node::EffSet { atoms } => {
            if atoms.is_empty() {
                out.push_str("(eff-set)");
            } else {
                out.push_str("(eff-set");
                for a in atoms {
                    out.push(' ');
                    out.push_str(a);
                }
                out.push(')');
            }
        }
        Node::EffVar { index } => {
            out.push_str("(eff-var ");
            out.push_str(&index.to_string());
            out.push(')');
        }
    }
}

fn emit_imports(entries: &[Node], out: &mut String) {
    if entries.is_empty() {
        out.push_str("(imports)");
        return;
    }
    let mut ordered: Vec<&Node> = entries.iter().collect();
    ordered.sort_by_key(|entry| import_hash_key(entry));
    out.push_str("(imports");
    for entry in ordered {
        out.push(' ');
        emit_into(entry, out);
    }
    out.push(')');
}

fn emit_import(hash: &str, sig: &Node, out: &mut String) {
    out.push_str("(imp ");
    emit_string(hash, out);
    out.push(' ');
    emit_into(sig, out);
    out.push(')');
}

fn emit_host_import(capability: &str, operation: &str, sig: &Node, out: &mut String) {
    out.push_str("(host-imp ");
    emit_string(capability, out);
    out.push(' ');
    emit_string(operation, out);
    out.push(' ');
    emit_into(sig, out);
    out.push(')');
}

fn emit_exports(entries: &[Node], out: &mut String) {
    if entries.is_empty() {
        out.push_str("(exports)");
        return;
    }
    let mut ordered: Vec<&Node> = entries.iter().collect();
    ordered.sort_by(|a, b| export_hash(a).cmp(export_hash(b)));
    out.push_str("(exports");
    for entry in ordered {
        out.push(' ');
        emit_into(entry, out);
    }
    out.push(')');
}

fn emit_export(visibility: &str, hash: &str, out: &mut String) {
    out.push_str("(exp ");
    out.push_str(visibility);
    out.push(' ');
    emit_string(hash, out);
    out.push(')');
}

fn emit_defs(defs: &[Node], out: &mut String) {
    let mut ordered: Vec<&Node> = defs.iter().collect();
    ordered.sort_by_key(|def| definition_hash_key(def));
    out.push_str("(defs");
    for def in ordered {
        out.push(' ');
        emit_into(def, out);
    }
    out.push(')');
}

fn emit_def(sig: &Node, body: &Node, out: &mut String) {
    out.push_str("(def ");
    emit_into(sig, out);
    out.push(' ');
    emit_into(body, out);
    out.push(')');
}

fn import_hash_key(node: &Node) -> [u8; 32] {
    match node {
        Node::Import { hash, .. } => hex_hash_key(hash),
        Node::HostImport { .. } => definition_hash_key(node),
        _ => [0; 32],
    }
}

fn export_hash(node: &Node) -> &str {
    match node {
        Node::Export { hash, .. } => hash.as_str(),
        _ => "",
    }
}

fn definition_hash_key(node: &Node) -> [u8; 32] {
    let bytes = emit(node);
    *blake3::hash(&bytes).as_bytes()
}

fn hex_hash_key(hash: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    if hash.len() != 64 {
        return out;
    }
    for (i, byte) in out.iter_mut().enumerate() {
        let hi = hex_nibble(hash.as_bytes()[i * 2]).unwrap_or(0);
        let lo = hex_nibble(hash.as_bytes()[i * 2 + 1]).unwrap_or(0);
        *byte = (hi << 4) | lo;
    }
    out
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Emit a canonical string literal (including surrounding quotes).
fn emit_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        let cp = ch as u32;
        match cp {
            0x22 => out.push_str("\\\""),
            0x5C => out.push_str("\\\\"),
            0x09 => out.push_str("\\t"),
            0x0A => out.push_str("\\n"),
            0x0D => out.push_str("\\r"),
            0x20..=0x7E => out.push(ch),
            // ADR 0010 S3: shortest lowercase hex, no leading zeros
            // (`\u{0}` for NUL is the one permitted single-zero form).
            _ => {
                out.push_str("\\u{");
                out.push_str(&format!("{:x}", cp));
                out.push('}');
            }
        }
    }
    out.push('"');
}
