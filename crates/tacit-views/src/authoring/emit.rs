//! Authoring-view emitter: Node + optional SidecarNode → BPE-compact text.
//!
//! Uses sidecar binder names when present; falls back to synthetic names per
//! sidecar-format.md § 5.  Projection rules: authoring-bpe-compact.md § "Direction 2".

use std::collections::BTreeMap;

use tacit_canonical::ast::Node;
use tacit_canonical::hash_node;

use crate::sidecar::SidecarNode;

/// Emit a Node as authoring-view text.
/// Pass `sidecar = None` to use fully synthetic names.
pub fn emit_authoring(node: &Node, sidecar: Option<&SidecarNode>) -> String {
    let mut out = String::new();
    let mut ctx = EmitCtx {
        stack: Vec::new(),
        lam_let_depth: 0,
        definition_aliases: sidecar.and_then(|s| s.definition_aliases.clone()),
        import_aliases: sidecar.and_then(|s| s.import_aliases.clone()),
        export_aliases: sidecar.and_then(|s| s.export_aliases.clone()),
    };
    ctx.emit_expr(node, sidecar, &mut out);
    out
}

struct EmitCtx {
    /// Names in scope, innermost last.
    stack: Vec<String>,
    /// Count of lam/let binders currently in scope (for v{n} synthetic names).
    lam_let_depth: usize,
    definition_aliases: Option<BTreeMap<String, String>>,
    import_aliases: Option<BTreeMap<String, String>>,
    export_aliases: Option<BTreeMap<String, String>>,
}

impl EmitCtx {
    fn emit_expr(&mut self, node: &Node, sc: Option<&SidecarNode>, out: &mut String) {
        match node {
            Node::Lam { body } => self.emit_lam(body, sc, out),
            Node::Let { rhs, body } => self.emit_let(rhs, body, sc, out),
            Node::Rec { bindings, body } => self.emit_rec(bindings, body, sc, out),
            Node::Module { bindings } => self.emit_module(bindings, sc, out),
            Node::Unit {
                imports,
                exports,
                defs,
            } => self.emit_unit(imports, exports, defs, sc, out),
            Node::If { cond, then, else_ } => self.emit_if(cond, then, else_, sc, out),
            Node::Match { scrutinee, arms } => self.emit_match(scrutinee, arms, sc, out),
            _ => self.emit_app_expr(node, sc, out),
        }
    }

    fn emit_lam(&mut self, body: &Node, sc: Option<&SidecarNode>, out: &mut String) {
        let name = sc
            .and_then(|s| s.binder.as_deref())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("v{}", self.lam_let_depth));
        out.push_str("lambda ");
        out.push_str(&name);
        out.push_str(". ");
        self.stack.push(name);
        let old_depth = self.lam_let_depth;
        self.lam_let_depth += 1;
        let body_sc = sc.and_then(|s| s.child(0));
        self.emit_expr(body, body_sc, out);
        self.lam_let_depth = old_depth;
        self.stack.pop();
    }

    fn emit_let(&mut self, rhs: &Node, body: &Node, sc: Option<&SidecarNode>, out: &mut String) {
        let name = sc
            .and_then(|s| s.binder.as_deref())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("v{}", self.lam_let_depth));

        // Increment depth before rhs so any lambdas inside rhs get unique names.
        let old_depth = self.lam_let_depth;
        self.lam_let_depth += 1;

        // Check if rhs is Ann (let x: T = V case).
        if let Node::Ann {
            expr: inner_rhs,
            type_,
        } = rhs
        {
            let ann_sc = sc.and_then(|s| s.child(0));
            let inner_rhs_sc = ann_sc.and_then(|s| s.child(0));
            let type_sc = ann_sc.and_then(|s| s.child(1));
            out.push_str("let ");
            out.push_str(&name);
            out.push(':');
            self.emit_expr(type_, type_sc, out);
            out.push_str(" = ");
            self.emit_expr(inner_rhs, inner_rhs_sc, out);
        } else {
            let rhs_sc = sc.and_then(|s| s.child(0));
            out.push_str("let ");
            out.push_str(&name);
            out.push_str(" = ");
            self.emit_expr(rhs, rhs_sc, out);
        }

        out.push_str(" in ");
        let body_sc = sc.and_then(|s| s.child(1));
        self.stack.push(name);
        self.emit_expr(body, body_sc, out);
        self.lam_let_depth = old_depth;
        self.stack.pop();
    }

    fn emit_rec(
        &mut self,
        bindings: &[Node],
        body: &Node,
        sc: Option<&SidecarNode>,
        out: &mut String,
    ) {
        let n = bindings.len();
        let names: Vec<String> = if let Some(binders) = sc.and_then(|s| s.binders.as_ref()) {
            binders.clone()
        } else {
            (0..n).map(|k| format!("B{}", k)).collect()
        };

        // Push names: N-1 first so names[0] ends up at top (= var 0).
        for name in names.iter().rev() {
            self.stack.push(name.clone());
        }

        out.push_str("rec {");
        for (k, (name, binding)) in names.iter().zip(bindings.iter()).enumerate() {
            if k > 0 {
                out.push_str("; ");
            }
            out.push_str(name);
            out.push_str(" = ");
            let b_sc = sc.and_then(|s| s.child(k));
            self.emit_expr(binding, b_sc, out);
        }
        out.push_str("} in ");

        let body_sc = sc.and_then(|s| s.child(n));
        self.emit_expr(body, body_sc, out);

        for _ in 0..n {
            self.stack.pop();
        }
    }

    fn emit_module(&mut self, bindings: &[Node], sc: Option<&SidecarNode>, out: &mut String) {
        let n = bindings.len();
        let names: Vec<String> = if let Some(binders) = sc.and_then(|s| s.binders.as_ref()) {
            binders.clone()
        } else {
            (0..n).map(|k| format!("B{}", k)).collect()
        };

        for name in names.iter().rev() {
            self.stack.push(name.clone());
        }

        out.push_str("module {");
        for (k, (name, binding)) in names.iter().zip(bindings.iter()).enumerate() {
            if k > 0 {
                out.push_str("; ");
            }
            out.push_str(name);
            out.push_str(" = ");
            let b_sc = sc.and_then(|s| s.child(k));
            self.emit_expr(binding, b_sc, out);
        }
        out.push('}');

        for _ in 0..n {
            self.stack.pop();
        }
    }

    fn emit_unit(
        &mut self,
        imports: &[Node],
        exports: &[Node],
        defs: &[Node],
        sc: Option<&SidecarNode>,
        out: &mut String,
    ) {
        let unit_alias = sc.and_then(|s| s.unit_alias.as_deref()).unwrap_or("Unit");
        out.push_str("unit ");
        out.push_str(unit_alias);
        out.push_str(" {");

        let mut first = true;
        let mut ordered_imports: Vec<&Node> = imports.iter().collect();
        ordered_imports.sort_by_key(|entry| unit_entry_hash(entry));
        for import in ordered_imports {
            if let Node::Import { hash, sig } = import {
                if !first {
                    out.push_str("; ");
                }
                first = false;
                let alias = self
                    .import_alias(hash)
                    .unwrap_or_else(|| synthetic_hash_name("import", hash));
                out.push_str("import ");
                out.push_str(&alias);
                out.push_str(" : ");
                emit_sig_type(sig, out);
                out.push_str(" from blake3:");
                out.push_str(hash);
            } else if let Node::HostImport {
                capability,
                operation,
                sig,
            } = import
            {
                let hash = hash_hex(import);
                if !first {
                    out.push_str("; ");
                }
                first = false;
                let alias = self
                    .import_alias(&hash)
                    .unwrap_or_else(|| synthetic_hash_name("host_import", &hash));
                out.push_str("import host ");
                out.push_str(&alias);
                out.push_str(" : ");
                emit_sig_type(sig, out);
                out.push_str(" from capability ");
                emit_string_literal(capability, out);
                out.push_str(" operation ");
                emit_string_literal(operation, out);
            }
        }

        let export_vis = export_visibility_map(exports);
        let mut ordered_defs: Vec<(String, &Node)> = def_map_by_hash(defs).into_iter().collect();
        ordered_defs.sort_by(|a, b| a.0.cmp(&b.0));
        for (hash, def) in ordered_defs {
            if let Node::Def { sig, body } = def {
                if !first {
                    out.push_str("; ");
                }
                first = false;
                if let Some(visibility) = export_vis.get(hash.as_str()) {
                    out.push_str("export ");
                    out.push_str(visibility);
                    out.push(' ');
                } else {
                    out.push_str("private ");
                }
                let alias = self
                    .definition_alias(hash.as_str())
                    .or_else(|| self.export_alias(hash.as_str()))
                    .unwrap_or_else(|| synthetic_hash_name("def", &hash));
                out.push_str(&alias);
                out.push_str(" : ");
                emit_sig_type(sig, out);
                out.push_str(" = ");
                self.emit_expr(body, sc, out);
            }
        }

        out.push('}');
    }

    fn emit_if(
        &mut self,
        cond: &Node,
        then: &Node,
        else_: &Node,
        sc: Option<&SidecarNode>,
        out: &mut String,
    ) {
        out.push_str("if ");
        self.emit_app_expr(cond, sc.and_then(|s| s.child(0)), out);
        out.push_str(" then ");
        self.emit_app_expr(then, sc.and_then(|s| s.child(1)), out);
        out.push_str(" else ");
        self.emit_expr(else_, sc.and_then(|s| s.child(2)), out);
    }

    fn emit_match(
        &mut self,
        scrutinee: &Node,
        arms: &[Node],
        sc: Option<&SidecarNode>,
        out: &mut String,
    ) {
        out.push_str("match ");
        self.emit_app_expr(scrutinee, sc.and_then(|s| s.child(0)), out);
        out.push_str(" with");
        for (i, arm) in arms.iter().enumerate() {
            out.push(' ');
            let arm_sc = sc.and_then(|s| s.child(i + 1));
            self.emit_arm(arm, arm_sc, out);
        }
    }

    fn emit_arm(&mut self, arm: &Node, sc: Option<&SidecarNode>, out: &mut String) {
        let Node::Arm { pattern, body } = arm else {
            out.push_str("| _ => _");
            return;
        };
        out.push_str("| ");
        let pat_sc = sc.and_then(|s| s.child(0));
        // Fresh counter per arm; synthetic names are p0, p1, …
        let mut counter = 0usize;
        let pat_vars = emit_pattern_counted(pattern, pat_sc, &mut counter, out);
        out.push_str(" => ");

        let save_len = self.stack.len();
        for name in &pat_vars {
            self.stack.push(name.clone());
        }
        let body_sc = sc.and_then(|s| s.child(1));
        self.emit_expr(body, body_sc, out);
        self.stack.truncate(save_len);
    }

    // -------------------------------------------------------------------------
    // Application-level and atoms
    // -------------------------------------------------------------------------

    fn emit_app_expr(&mut self, node: &Node, sc: Option<&SidecarNode>, out: &mut String) {
        match node {
            Node::App { fn_, arg } => {
                // App sidecar children: [fn_sc, arg_sc].
                let fn_sc = sc.and_then(|s| s.child(0));
                let arg_sc = sc.and_then(|s| s.child(1));
                // Function position: another App is fine (left-assoc); structural forms need parens.
                if needs_parens_as_fn(fn_) {
                    out.push('(');
                    self.emit_expr(fn_, fn_sc, out);
                    out.push(')');
                } else {
                    self.emit_app_expr(fn_, fn_sc, out);
                }
                out.push(' ');
                if needs_parens_as_arg(arg) {
                    out.push('(');
                    self.emit_expr(arg, arg_sc, out);
                    out.push(')');
                } else {
                    self.emit_proj_atom(arg, arg_sc, out);
                }
            }
            Node::Ctor { name, args } => {
                out.push_str(name);
                for (k, arg) in args.iter().enumerate() {
                    out.push(' ');
                    // Ctor sidecar has no leading null on the value side; children are arg entries.
                    let arg_sc = sc.and_then(|s| s.child(k));
                    if needs_parens_as_arg(arg) {
                        out.push('(');
                        self.emit_expr(arg, arg_sc, out);
                        out.push(')');
                    } else {
                        self.emit_proj_atom(arg, arg_sc, out);
                    }
                }
            }
            _ => self.emit_proj_atom(node, sc, out),
        }
    }

    fn emit_proj_atom(&mut self, node: &Node, sc: Option<&SidecarNode>, out: &mut String) {
        match node {
            Node::Proj { record, field } => {
                let rec_sc = sc.and_then(|s| s.child(0));
                // record needs parens if it's a structural form (rare but possible).
                if needs_parens_as_fn(record) {
                    out.push('(');
                    self.emit_expr(record, rec_sc, out);
                    out.push(')');
                } else {
                    self.emit_proj_atom(record, rec_sc, out);
                }
                out.push('.');
                out.push_str(field);
            }
            _ => self.emit_atom(node, sc, out),
        }
    }

    fn emit_atom(&mut self, node: &Node, sc: Option<&SidecarNode>, out: &mut String) {
        match node {
            Node::Var { index } => {
                let i = *index as usize;
                let len = self.stack.len();
                if i < len {
                    out.push_str(&self.stack[len - 1 - i]);
                } else {
                    // Index out of range; emit a placeholder.
                    out.push_str(&format!("?var{}", index));
                }
            }
            Node::Int { value } => out.push_str(value),
            Node::Str { value } => {
                out.push('"');
                for ch in value.chars() {
                    match ch {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\t' => out.push_str("\\t"),
                        '\r' => out.push_str("\\r"),
                        c if (c as u32) < 0x20 || c as u32 == 0x7F => {
                            out.push_str(&format!("\\u{{{:x}}}", c as u32));
                        }
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Node::Sym { name } => {
                out.push('@');
                out.push_str(name);
            }
            Node::Ref { hash } => {
                if let Some(alias) = self.ref_alias(hash) {
                    out.push_str(&alias);
                } else {
                    out.push_str("blake3:");
                    out.push_str(hash);
                }
            }
            Node::Hole { .. } | Node::PatWild | Node::PatVar => out.push('_'),
            Node::Record { fields } => {
                out.push('{');
                let n = fields.len();
                // field_order[authoring_pos] = canonical_index; emit in authoring order.
                let emit_order: Vec<usize> =
                    if let Some(fo) = sc.and_then(|s| s.field_order.as_ref()) {
                        fo.clone()
                    } else {
                        (0..n).collect()
                    };
                for (display_pos, &canon_idx) in emit_order.iter().enumerate() {
                    if display_pos > 0 {
                        out.push_str(", ");
                    }
                    if canon_idx < fields.len() {
                        let (key, val) = &fields[canon_idx];
                        out.push_str(key);
                        out.push(':');
                        // Child index in sidecar for records: 2*canon_idx+1 (val after sym).
                        let val_sc = sc.and_then(|s| s.child(2 * canon_idx + 1));
                        self.emit_as_arg(val, val_sc, out);
                    }
                }
                out.push('}');
            }
            Node::Ann { expr, type_ } => {
                // Standalone ann: (E : T)
                out.push('(');
                self.emit_expr(expr, sc.and_then(|s| s.child(0)), out);
                out.push(':');
                self.emit_expr(type_, sc.and_then(|s| s.child(1)), out);
                out.push(')');
            }
            Node::Ctor { name, args } if args.is_empty() => {
                out.push_str(name);
            }
            // Phase 2 type-expression nodes have no authoring syntax yet (Stage 5 will add it).
            // Emit as canonical text to avoid infinite recursion through the emit_atom fallback.
            Node::FnTy { .. }
            | Node::TyVar { .. }
            | Node::Forall { .. }
            | Node::EffSet { .. }
            | Node::EffVar { .. }
            | Node::Imports { .. }
            | Node::Import { .. }
            | Node::HostImport { .. }
            | Node::Exports { .. }
            | Node::Export { .. }
            | Node::Defs { .. }
            | Node::Def { .. }
            | Node::Sig { .. } => {
                let canonical = tacit_canonical::emit::emit(node);
                out.push_str(&String::from_utf8_lossy(&canonical));
            }
            // Structural forms that shouldn't appear as bare atoms — wrap in parens.
            other => {
                out.push('(');
                self.emit_expr(other, sc, out);
                out.push(')');
            }
        }
    }

    /// Emit a node in argument position (parens if non-atomic).
    fn emit_as_arg(&mut self, node: &Node, sc: Option<&SidecarNode>, out: &mut String) {
        if needs_parens_as_arg(node) {
            out.push('(');
            self.emit_expr(node, sc, out);
            out.push(')');
        } else {
            self.emit_proj_atom(node, sc, out);
        }
    }

    fn ref_alias(&self, hash: &str) -> Option<String> {
        self.import_alias(hash)
            .or_else(|| self.definition_alias(hash))
            .or_else(|| self.export_alias(hash))
    }

    fn import_alias(&self, hash: &str) -> Option<String> {
        self.alias_from_map(&self.import_aliases, hash)
    }

    fn definition_alias(&self, hash: &str) -> Option<String> {
        self.alias_from_map(&self.definition_aliases, hash)
    }

    fn export_alias(&self, hash: &str) -> Option<String> {
        self.alias_from_map(&self.export_aliases, hash)
    }

    fn alias_from_map(&self, map: &Option<BTreeMap<String, String>>, hash: &str) -> Option<String> {
        let alias = map.as_ref()?.get(hash)?;
        self.alias_is_unambiguous(hash, alias)
            .then(|| alias.clone())
    }

    fn alias_is_unambiguous(&self, hash: &str, alias: &str) -> bool {
        for map in [
            &self.import_aliases,
            &self.definition_aliases,
            &self.export_aliases,
        ] {
            let Some(map) = map else {
                continue;
            };
            for (candidate_hash, candidate_alias) in map {
                if candidate_alias == alias && candidate_hash != hash {
                    return false;
                }
            }
        }
        true
    }
}

// -------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------

/// Emit a pattern node, counting pat-vars to generate synthetic p0, p1, … names.
/// Returns pat-var names in textual order (same order they were pushed: first → last).
fn emit_pattern_counted(
    node: &Node,
    sc: Option<&SidecarNode>,
    counter: &mut usize,
    out: &mut String,
) -> Vec<String> {
    match node {
        Node::PatWild => {
            out.push('_');
            vec![]
        }
        Node::PatVar => {
            let name = sc
                .and_then(|s| s.binder.as_deref())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("p{}", counter));
            *counter += 1;
            out.push_str(&name);
            vec![name]
        }
        Node::PatCtor { name, sub_patterns } => {
            out.push_str(name);
            let mut all_vars: Vec<String> = Vec::new();
            // Sidecar: child(0) = null (ctor-name sym), child(k+1) = sub-pat k.
            for (k, sp) in sub_patterns.iter().enumerate() {
                out.push(' ');
                let sp_sc = sc.and_then(|s| s.child(k + 1));
                let vars = emit_pattern_counted(sp, sp_sc, counter, out);
                all_vars.extend(vars);
            }
            all_vars
        }
        Node::PatInt { value } => {
            out.push_str(value);
            vec![]
        }
        _ => {
            out.push('_');
            vec![]
        }
    }
}

fn needs_parens_as_fn(node: &Node) -> bool {
    matches!(
        node,
        Node::Lam { .. }
            | Node::Let { .. }
            | Node::Rec { .. }
            | Node::Module { .. }
            | Node::Unit { .. }
            | Node::If { .. }
            | Node::Match { .. }
            | Node::Ann { .. }
    )
}

fn needs_parens_as_arg(node: &Node) -> bool {
    match node {
        Node::App { .. }
        | Node::Lam { .. }
        | Node::Let { .. }
        | Node::Rec { .. }
        | Node::Module { .. }
        | Node::Unit { .. }
        | Node::If { .. }
        | Node::Match { .. }
        | Node::Ann { .. } => true,
        Node::Ctor { args, .. } => !args.is_empty(),
        _ => false,
    }
}

fn unit_entry_hash(node: &Node) -> String {
    match node {
        Node::Import { hash, .. } | Node::Export { hash, .. } | Node::Ref { hash } => hash.clone(),
        Node::HostImport { .. } => hash_hex(node),
        _ => String::new(),
    }
}

fn hash_hex(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

fn emit_string_literal(value: &str, out: &mut String) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7F => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn def_map_by_hash(defs: &[Node]) -> BTreeMap<String, &Node> {
    defs.iter()
        .filter_map(|def| match def {
            Node::Def { .. } => {
                let hash = tacit_canonical::hash_node(def);
                let hex = hash.iter().map(|b| format!("{:02x}", b)).collect();
                Some((hex, def))
            }
            _ => None,
        })
        .collect()
}

fn export_visibility_map(exports: &[Node]) -> BTreeMap<String, String> {
    exports
        .iter()
        .filter_map(|export| match export {
            Node::Export { visibility, hash } => Some((hash.clone(), visibility.clone())),
            _ => None,
        })
        .collect()
}

fn synthetic_hash_name(prefix: &str, hash: &str) -> String {
    let short = if hash.len() >= 8 { &hash[..8] } else { hash };
    format!("{}_{}", prefix, short)
}

fn emit_sig_type(sig: &Node, out: &mut String) {
    match sig {
        Node::Sig { type_, .. } => emit_type(type_, out),
        other => emit_type(other, out),
    }
}

fn emit_type(node: &Node, out: &mut String) {
    match node {
        Node::Sym { name } => out.push_str(name),
        Node::FnTy { arg, ret, eff } => {
            let parens = matches!(arg.as_ref(), Node::FnTy { .. });
            if parens {
                out.push('(');
            }
            emit_type(arg, out);
            if parens {
                out.push(')');
            }
            out.push_str(" -> ");
            emit_type(ret, out);
            if let Node::EffSet { atoms } = eff.as_ref() {
                if !atoms.is_empty() {
                    out.push_str(" / {");
                    out.push_str(&atoms.join(", "));
                    out.push('}');
                }
            }
        }
        Node::Record { fields } => {
            out.push('{');
            for (i, (name, ty)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(name);
                out.push_str(": ");
                emit_type(ty, out);
            }
            out.push('}');
        }
        other => {
            let canonical = tacit_canonical::emit::emit(other);
            out.push_str(&String::from_utf8_lossy(&canonical));
        }
    }
}
