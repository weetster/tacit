//! Inspection-view renderer (display-only; not round-trippable).
//!
//! Implements the per-kind rendering rules from plans/inspection-view.md,
//! with optional L1 (--debruijn) and L2 (--hashes) annotation overlays.
//! Reference: decisions/0015-inspection-view-scope.md.

use std::collections::{BTreeMap, BTreeSet};

use tacit_canonical::ast::Node;
use tacit_canonical::hash_node;

use crate::sidecar::SidecarNode;

#[derive(Default)]
pub struct InspectFlags {
    pub debruijn: bool,
    pub hashes: bool,
    /// (inspection) Render type annotations in human-readable form (Phase 2).
    pub types: bool,
    /// (inspection) Render effect annotations verbosely (Phase 2).
    pub effects: bool,
}

/// Render `node` to inspection-view text at the top level (indent 0).
pub fn emit_inspection(node: &Node, sidecar: Option<&SidecarNode>, flags: &InspectFlags) -> String {
    let mut ctx = Ctx {
        stack: Vec::new(),
        lam_let_depth: 0,
        pat_var_counter: 0,
        definition_aliases: sidecar.and_then(|s| s.definition_aliases.clone()),
        import_aliases: sidecar.and_then(|s| s.import_aliases.clone()),
        export_aliases: sidecar.and_then(|s| s.export_aliases.clone()),
        flags,
    };
    let r = ctx.render(node, sidecar, 0);
    if r.inline {
        Ctx::annotate(&r.text, &r.annots, flags)
    } else {
        r.text
    }
}

// ---------------------------------------------------------------------------

/// A rendered subtree: its text and whether it is "inline" (no newlines).
struct Rendered {
    text: String,
    inline: bool,
    /// L1 var annotations (name, debruijn_index) from inline var leaves.
    /// Only meaningful when inline == true; empty for always-break kinds.
    annots: Vec<(String, u64)>,
}

impl Rendered {
    fn leaf(text: impl Into<String>) -> Self {
        Rendered {
            text: text.into(),
            inline: true,
            annots: Vec::new(),
        }
    }

    fn always_break(text: impl Into<String>) -> Self {
        Rendered {
            text: text.into(),
            inline: false,
            annots: Vec::new(),
        }
    }
}

struct Ctx<'f> {
    /// Binding stack; innermost binder is at the back. var 0 = stack[len-1].
    stack: Vec<String>,
    /// Number of lam/let binders currently in scope, for synthetic v{n} names.
    lam_let_depth: usize,
    /// Resets to 0 at each arm; used for synthetic p{n} pat-var names.
    pat_var_counter: usize,
    definition_aliases: Option<BTreeMap<String, String>>,
    import_aliases: Option<BTreeMap<String, String>>,
    export_aliases: Option<BTreeMap<String, String>>,
    flags: &'f InspectFlags,
}

impl<'f> Ctx<'f> {
    // -----------------------------------------------------------------------
    // Utilities
    // -----------------------------------------------------------------------

    fn pad(n: usize) -> String {
        " ".repeat(n)
    }

    fn resolve_var(&self, index: u64) -> String {
        let i = index as usize;
        let len = self.stack.len();
        if i < len {
            self.stack[len - 1 - i].clone()
        } else {
            format!("?var{}", index)
        }
    }

    fn hash_badge(node: &Node) -> String {
        let h = hash_node(node);
        let hex: String = h.iter().take(4).map(|b| format!("{:02x}", b)).collect();
        format!("[{}]", hex)
    }

    fn badge(&self, node: &Node, r: Rendered) -> Rendered {
        let b = Self::hash_badge(node);
        Rendered {
            text: format!("{} {}", b, r.text),
            inline: r.inline,
            annots: r.annots,
        }
    }

    fn annotate(text: &str, annots: &[(String, u64)], flags: &InspectFlags) -> String {
        if !flags.debruijn || annots.is_empty() {
            return text.to_string();
        }
        let comment = annots
            .iter()
            .map(|(name, idx)| format!("{} \u{2261} var {}", name, idx))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}  # {}", text, comment)
    }

    // -----------------------------------------------------------------------
    // Dispatch
    // -----------------------------------------------------------------------

    fn render(&mut self, node: &Node, sc: Option<&SidecarNode>, indent: usize) -> Rendered {
        // Check for a sidecar comment and prepend it.
        let comment = sc.and_then(|s| s.comment.as_deref());

        let r = match node {
            Node::Lam { body } => self.render_lam(node, body, sc, indent),
            Node::App { fn_, arg } => self.render_app(node, fn_, arg, sc, indent),
            Node::Let { rhs, body } => self.render_let(node, rhs, body, sc, indent),
            Node::Rec { bindings, body } => self.render_rec(node, bindings, body, sc, indent),
            Node::Module { bindings } => self.render_module(node, bindings, sc, indent),
            Node::Unit {
                imports,
                exports,
                defs,
            } => self.render_unit(node, imports, exports, defs, sc, indent),
            Node::Imports { entries } => self.render_imports(entries, sc, indent),
            Node::Import { hash, sig } => {
                let alias = self
                    .import_alias(hash)
                    .unwrap_or_else(|| synthetic_hash_name("import", hash));
                Rendered::leaf(format!(
                    "{} : {} = {}",
                    alias,
                    format_signature(sig),
                    hash_text(hash, self.flags.hashes)
                ))
            }
            Node::HostImport {
                capability,
                operation,
                sig,
            } => {
                let hash = hash_hex(node);
                let alias = self
                    .import_alias(&hash)
                    .unwrap_or_else(|| synthetic_hash_name("host_import", &hash));
                Rendered::leaf(format!(
                    "host {} : {} from capability {} operation {} = {}",
                    alias,
                    format_signature(sig),
                    render_string_inline(capability),
                    render_string_inline(operation),
                    hash_text(&hash, self.flags.hashes)
                ))
            }
            Node::Exports { entries } => self.render_exports(entries, sc, indent),
            Node::Export { visibility, hash } => {
                let alias = self
                    .export_alias(hash)
                    .or_else(|| self.definition_alias(hash))
                    .unwrap_or_else(|| synthetic_hash_name("export", hash));
                Rendered::leaf(format!(
                    "{} {} = {}",
                    visibility,
                    alias,
                    hash_text(hash, self.flags.hashes)
                ))
            }
            Node::Defs { defs } => self.render_defs(defs, sc, indent),
            Node::Def { sig, body } => self.render_def(sig, body, sc, indent),
            Node::State { name, type_ } => {
                Rendered::leaf(format!("state {} = {}", name, format_type_node_nice(type_)))
            }
            Node::Sig { type_, eval_eff } => {
                Rendered::leaf(format_signature_parts(type_, eval_eff))
            }
            Node::Ref { hash } => {
                let alias = self.alias_for_ref(hash);
                Rendered::leaf(alias)
            }
            Node::If { cond, then, else_ } => self.render_if(node, cond, then, else_, sc, indent),
            Node::Match { scrutinee, arms } => self.render_match(node, scrutinee, arms, sc, indent),
            Node::Arm { pattern, body } => self.render_arm(node, pattern, body, sc, indent),
            Node::Record { fields } => self.render_record(node, fields, sc, indent),
            Node::Proj { record, field } => self.render_proj(node, record, field, sc, indent),
            Node::Ctor { name, args } => self.render_ctor(node, name, args, sc, indent),
            Node::Ann { expr, type_ } => self.render_ann(node, expr, type_, sc, indent),
            Node::Var { index } => self.render_var(*index),
            Node::Int { value } => Rendered::leaf(value.clone()),
            Node::Str { value } => Self::render_str(value),
            Node::Sym { name } => Rendered::leaf(format!("@{}", name)),
            Node::Hole { diag_id, payload } => self.render_hole(diag_id, payload, sc, indent),
            Node::PatWild => Rendered::leaf("_"),
            Node::PatVar => self.render_pat_var(sc),
            Node::PatCtor { name, sub_patterns } => {
                let (text, _vars) = self.render_pattern_node(node, name, sub_patterns, sc);
                Rendered::leaf(text)
            }
            // Phase 2 type-level nodes.
            // --types / --effects flags (ADR 0015) render these verbosely.
            Node::PatInt { value } => Rendered::leaf(value.clone()),
            Node::FnTy { arg, ret, eff } => {
                if self.flags.types {
                    let arg_s = format_type_node_nice(arg);
                    let ret_s = format_type_node_nice(ret);
                    let eff_s = format_eff_node_nice(eff, true);
                    let needs_parens =
                        matches!(arg.as_ref(), Node::FnTy { .. } | Node::Forall { .. });
                    let text = if needs_parens {
                        format!("({}) -> {}{}", arg_s, ret_s, eff_s)
                    } else {
                        format!("{} -> {}{}", arg_s, ret_s, eff_s)
                    };
                    Rendered::leaf(text)
                } else {
                    Rendered::leaf(format!(
                        "({} -> {} / {})",
                        self.render_compact(arg),
                        self.render_compact(ret),
                        self.render_compact(eff)
                    ))
                }
            }
            Node::TyVar { index } => {
                if self.flags.types {
                    Rendered::leaf(format!("\u{03b1}{}", index))
                } else {
                    Rendered::leaf(format!("ty-var({})", index))
                }
            }
            Node::Forall {
                ty_count,
                eff_count,
                body,
            } => {
                if self.flags.types {
                    Rendered::leaf(format_forall_nice(*ty_count, *eff_count, body))
                } else {
                    Rendered::leaf(format!(
                        "forall({},{},{})",
                        ty_count,
                        eff_count,
                        self.render_compact(body)
                    ))
                }
            }
            Node::EffSet { atoms } => {
                if atoms.is_empty() {
                    Rendered::leaf("{}".to_string())
                } else if self.flags.effects {
                    Rendered::leaf(format!("{{{}}}", atoms.join(", ")))
                } else {
                    Rendered::leaf(format!("{{{}}}", atoms.join(",")))
                }
            }
            Node::EffVar { index } => {
                if self.flags.effects {
                    Rendered::leaf(format!("\u{03b5}{}", index))
                } else {
                    Rendered::leaf(format!("eff-var({})", index))
                }
            }
        };

        if let Some(c) = comment {
            prepend_comment(r, c, indent)
        } else {
            r
        }
    }

    /// Render a type-level node as a compact single-line string.
    /// Used for Phase 2 type annotations before --types flag is wired (Stage 5).
    fn render_compact(&self, node: &Node) -> String {
        use tacit_canonical::emit::emit;
        String::from_utf8_lossy(&emit(node)).into_owned()
    }

    fn alias_for_ref(&self, hash: &str) -> String {
        self.import_alias(hash)
            .or_else(|| self.definition_alias(hash))
            .or_else(|| self.export_alias(hash))
            .unwrap_or_else(|| hash_text(hash, self.flags.hashes))
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

    // -----------------------------------------------------------------------
    // Leaves
    // -----------------------------------------------------------------------

    fn render_var(&self, index: u64) -> Rendered {
        let name = self.resolve_var(index);
        let annots = if self.flags.debruijn {
            vec![(name.clone(), index)]
        } else {
            Vec::new()
        };
        Rendered {
            text: name,
            inline: true,
            annots,
        }
    }

    fn render_str(value: &str) -> Rendered {
        let mut out = String::from('"');
        for ch in value.chars() {
            let cp = ch as u32;
            match cp {
                0x22 => out.push_str("\\\""),
                0x5C => out.push_str("\\\\"),
                0x09 => out.push_str("\\t"),
                0x0A => out.push_str("\\n"),
                0x0D => out.push_str("\\r"),
                0x20..=0x7E => out.push(ch),
                _ => out.push_str(&format!("\\u{{{:x}}}", cp)),
            }
        }
        out.push('"');
        Rendered::leaf(out)
    }

    fn render_hole(
        &mut self,
        diag_id: &str,
        payload: &Node,
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        // hole canonical children: [diag-id-sym, payload]; payload sidecar is child(1).
        let payload_sc = sc.and_then(|s| s.child(1));
        let payload_r = self.render(payload, payload_sc, indent);
        // Holes are always inline (§ 3.10) — squash newlines so multi-line
        // payloads collapse onto a single visual line.
        let payload_str = payload_r.text.replace('\n', " ");
        // U+27E8 / U+27E9 mathematical angle brackets
        Rendered::leaf(format!("\u{27E8}hole:{} {}\u{27E9}", diag_id, payload_str))
    }

    fn render_pat_var(&mut self, sc: Option<&SidecarNode>) -> Rendered {
        let name = sc
            .and_then(|s| s.binder.as_deref())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("p{}", self.pat_var_counter));
        self.pat_var_counter += 1;
        Rendered::leaf(name)
    }

    // -----------------------------------------------------------------------
    // Lam — inline iff body is inline; collapse nested lam chains.
    // -----------------------------------------------------------------------

    fn render_lam(
        &mut self,
        node: &Node,
        body: &Node,
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        // Collect the whole lambda chain before pushing anything.
        let mut params: Vec<String> = Vec::new();
        let mut cur_lam_sc = sc;
        let mut cur_body: &Node = body;
        let final_body_sc: Option<&SidecarNode>;

        loop {
            let depth_offset = params.len();
            let name = cur_lam_sc
                .and_then(|s| s.binder.as_deref())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("v{}", self.lam_let_depth + depth_offset));
            params.push(name);
            let body_sc = cur_lam_sc.and_then(|s| s.child(0));
            match cur_body {
                Node::Lam { body: inner } => {
                    cur_lam_sc = body_sc;
                    cur_body = inner;
                }
                _ => {
                    final_body_sc = body_sc;
                    break;
                }
            }
        }

        let capture_names = self.capture_names(cur_body, params.len() as u64);

        // Push all params (outermost first → outermost ends up deepest in stack).
        let old_depth = self.lam_let_depth;
        for name in &params {
            self.stack.push(name.clone());
            self.lam_let_depth += 1;
        }

        let body_r = self.render(cur_body, final_body_sc, indent + 2);

        for _ in &params {
            self.stack.pop();
        }
        self.lam_let_depth = old_depth;

        let param_str = params.join(" ");
        let capture_suffix =
            if (self.flags.types || self.flags.effects) && !capture_names.is_empty() {
                format!(" [captures {}]", capture_names.join(", "))
            } else {
                String::new()
            };
        let result = if body_r.inline {
            // Propagate annots up; a parent line-builder (let header, arm body,
            // top-level emit, …) appends the trailing comment so it lands at the
            // end of the assembled line — keeping `in`/`=>` outside the comment.
            let text = format!("lambda {}{}. {}", param_str, capture_suffix, body_r.text);
            Rendered {
                text,
                inline: true,
                annots: body_r.annots,
            }
        } else {
            let text = format!(
                "lambda {}{}.\n{}{}",
                param_str,
                capture_suffix,
                Self::pad(indent + 2),
                body_r.text
            );
            Rendered::always_break(text)
        };

        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    fn capture_names(&self, body: &Node, depth: u64) -> Vec<String> {
        let mut free = BTreeSet::new();
        collect_free_outer_indices(body, depth, &mut free);
        let len = self.stack.len();
        free.into_iter()
            .map(|outer_index| {
                if outer_index < len {
                    self.stack[len - 1 - outer_index].clone()
                } else {
                    format!("?var{}", outer_index as u64 + depth)
                }
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // App — flatten left-associative spine; inline iff all parts inline.
    // -----------------------------------------------------------------------

    fn render_app(
        &mut self,
        node: &Node,
        fn_: &Node,
        arg: &Node,
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        // Collect the full left spine: [(head, head_sc), arg1, arg2, …]
        let mut args: Vec<(&Node, Option<&SidecarNode>)> = Vec::new();
        let mut cur_fn: &Node = fn_;
        let mut cur_fn_sc = sc.and_then(|s| s.child(0));
        args.push((arg, sc.and_then(|s| s.child(1))));

        while let Node::App { fn_: f2, arg: a2 } = cur_fn {
            let f2_sc = cur_fn_sc.and_then(|s| s.child(0));
            let a2_sc = cur_fn_sc.and_then(|s| s.child(1));
            args.push((a2, a2_sc));
            cur_fn = f2;
            cur_fn_sc = f2_sc;
        }
        args.reverse(); // args[0] = first argument, args[N-1] = last argument

        if let Node::Sym { name } = cur_fn {
            if let Some(labels) = combinator_labels(name, args.len()) {
                let result = self.render_combinator_app(name, labels, &args, indent);
                return if self.flags.hashes {
                    self.badge(node, result)
                } else {
                    result
                };
            }
        }

        let head_r = self.render(cur_fn, cur_fn_sc, indent);
        let mut arg_rs: Vec<Rendered> = Vec::new();
        for (a, a_sc) in &args {
            let ar = self.render_as_arg(a, *a_sc, indent + 2);
            arg_rs.push(ar);
        }

        let all_inline = head_r.inline && arg_rs.iter().all(|r| r.inline);
        let result = if all_inline {
            let parts: Vec<&str> = std::iter::once(head_r.text.as_str())
                .chain(arg_rs.iter().map(|r| r.text.as_str()))
                .collect();
            let text = parts.join(" ");
            // Propagate annots; the parent line-builder appends the trailing
            // comment after any structural keyword (`in`, `=>`, `then`, …).
            let annots: Vec<_> = head_r
                .annots
                .iter()
                .chain(arg_rs.iter().flat_map(|r| r.annots.iter()))
                .cloned()
                .collect();
            Rendered {
                text,
                inline: true,
                annots,
            }
        } else {
            let mut text = head_r.text.clone();
            for ar in &arg_rs {
                text.push('\n');
                text.push_str(&Self::pad(indent + 2));
                text.push_str(&ar.text);
            }
            Rendered::always_break(text)
        };

        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    fn render_combinator_app(
        &mut self,
        name: &str,
        labels: &[&str],
        args: &[(&Node, Option<&SidecarNode>)],
        indent: usize,
    ) -> Rendered {
        let mut text = format!("@{}", name);
        for (label, (arg, arg_sc)) in labels.iter().zip(args.iter()) {
            let arg_r = self.render(arg, *arg_sc, indent + 4);
            text.push('\n');
            text.push_str(&Self::pad(indent + 2));
            if arg_r.inline {
                let line = format!("{} {}", label, arg_r.text);
                text.push_str(&Self::annotate(&line, &arg_r.annots, self.flags));
            } else {
                text.push_str(label);
                text.push('\n');
                text.push_str(&Self::pad(indent + 4));
                text.push_str(&arg_r.text);
            }
        }
        Rendered::always_break(text)
    }

    /// Render a node in argument position, wrapping in parens when needed.
    fn render_as_arg(&mut self, node: &Node, sc: Option<&SidecarNode>, indent: usize) -> Rendered {
        if self.needs_parens_as_arg(node) {
            let inner = self.render(node, sc, indent + 1);
            Rendered {
                text: format!("({})", inner.text),
                inline: inner.inline,
                annots: inner.annots,
            }
        } else {
            self.render(node, sc, indent)
        }
    }

    fn needs_parens_as_arg(&self, node: &Node) -> bool {
        match node {
            Node::App { .. }
            | Node::Lam { .. }
            | Node::Let { .. }
            | Node::Rec { .. }
            | Node::If { .. }
            | Node::Match { .. }
            | Node::Ann { .. } => true,
            Node::Ctor { args, .. } => !args.is_empty(),
            _ => false,
        }
    }

    // -----------------------------------------------------------------------
    // Let — always-break; handles Ann rhs and chained let bodies.
    // -----------------------------------------------------------------------

    fn render_let(
        &mut self,
        node: &Node,
        rhs: &Node,
        body: &Node,
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        let name = sc
            .and_then(|s| s.binder.as_deref())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("v{}", self.lam_let_depth));

        let old_depth = self.lam_let_depth;
        self.lam_let_depth += 1;

        // Header: "let X = RHS" or "let X: T = RHS"
        let (header, rhs_inline, rhs_annots) = if let Node::Ann { expr, type_ } = rhs {
            let ann_sc = sc.and_then(|s| s.child(0));
            let inner_rhs_sc = ann_sc.and_then(|s| s.child(0));
            let _type_sc = ann_sc.and_then(|s| s.child(1));
            let type_r = Rendered::leaf(format_type_node_nice(type_));
            let val_r = self.render(expr, inner_rhs_sc, indent + 2);
            let inline = type_r.inline && val_r.inline;
            let header = if inline {
                format!("let {}: {} = {}", name, type_r.text, val_r.text)
            } else {
                // Non-inline: put them on their indented lines
                if type_r.inline {
                    format!(
                        "let {}: {} =\n{}{}",
                        name,
                        type_r.text,
                        Self::pad(indent + 2),
                        val_r.text
                    )
                } else {
                    format!(
                        "let {}: =\n{}{}\n{}= {}",
                        name,
                        Self::pad(indent + 2),
                        type_r.text,
                        Self::pad(indent),
                        val_r.text
                    )
                }
            };
            let annots: Vec<_> = type_r.annots.into_iter().chain(val_r.annots).collect();
            (header, inline, annots)
        } else {
            let rhs_sc = sc.and_then(|s| s.child(0));
            let rhs_r = self.render(rhs, rhs_sc, indent + 2);
            let inline = rhs_r.inline;
            let header = if inline {
                format!("let {} = {}", name, rhs_r.text)
            } else {
                format!("let {} =\n{}{}", name, Self::pad(indent + 2), rhs_r.text)
            };
            (header, inline, rhs_r.annots)
        };

        // Push name for body
        self.stack.push(name);

        // Chain: if body is also a let, render at the same indent (no extra nesting).
        let body_sc = sc.and_then(|s| s.child(1));
        let body_is_let = matches!(body, Node::Let { .. });
        let body_indent = if body_is_let { indent } else { indent + 2 };
        let body_r = self.render(body, body_sc, body_indent);

        self.lam_let_depth = old_depth;
        self.stack.pop();

        // Body line: when the body is inline, append the L1 annotation here
        // (so it follows the body text on its own line).
        let body_line = if body_r.inline {
            Self::annotate(&body_r.text, &body_r.annots, self.flags)
        } else {
            body_r.text
        };

        let text = if rhs_inline {
            let header_line = Self::annotate(&format!("{} in", header), &rhs_annots, self.flags);
            format!("{}\n{}{}", header_line, Self::pad(body_indent), body_line)
        } else {
            // "let X =\n  V\nin\n  B"
            format!(
                "{}\n{}in\n{}{}",
                header,
                Self::pad(indent),
                Self::pad(body_indent),
                body_line
            )
        };

        let result = Rendered::always_break(text);
        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    // -----------------------------------------------------------------------
    // Rec — always-break.
    // -----------------------------------------------------------------------

    fn render_rec(
        &mut self,
        node: &Node,
        bindings: &[Node],
        body: &Node,
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        let n = bindings.len();
        let names: Vec<String> = sc
            .and_then(|s| s.binders.as_ref())
            .cloned()
            .unwrap_or_else(|| (0..n).map(|k| format!("B{}", k)).collect());

        // Push names: reverse so names[0] lands at stack top (= var 0).
        for name in names.iter().rev() {
            self.stack.push(name.clone());
        }

        let pipe_indent = indent + 2;
        let rhs_break_indent = indent + 6;

        let mut text = String::from("rec");
        for (k, (name, binding)) in names.iter().zip(bindings.iter()).enumerate() {
            let b_sc = sc.and_then(|s| s.child(k));
            let rhs_r = self.render(binding, b_sc, rhs_break_indent);
            text.push('\n');
            text.push_str(&Self::pad(pipe_indent));
            if rhs_r.inline {
                let line = format!("| {} = {}", name, rhs_r.text);
                text.push_str(&Self::annotate(&line, &rhs_r.annots, self.flags));
            } else {
                text.push_str(&format!(
                    "| {} =\n{}{}",
                    name,
                    Self::pad(rhs_break_indent),
                    rhs_r.text
                ));
            }
        }

        let body_sc = sc.and_then(|s| s.child(n));
        let body_r = self.render(body, body_sc, indent + 2);

        for _ in 0..n {
            self.stack.pop();
        }

        let body_line = if body_r.inline {
            Self::annotate(&body_r.text, &body_r.annots, self.flags)
        } else {
            body_r.text
        };

        text.push('\n');
        text.push_str(&Self::pad(indent));
        text.push_str("in");
        text.push('\n');
        text.push_str(&Self::pad(indent + 2));
        text.push_str(&body_line);

        let result = Rendered::always_break(text);
        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    // -----------------------------------------------------------------------
    // Module — always-break (no body).
    // -----------------------------------------------------------------------

    fn render_module(
        &mut self,
        node: &Node,
        bindings: &[Node],
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        let n = bindings.len();
        let names: Vec<String> = sc
            .and_then(|s| s.binders.as_ref())
            .cloned()
            .unwrap_or_else(|| (0..n).map(|k| format!("B{}", k)).collect());

        for name in names.iter().rev() {
            self.stack.push(name.clone());
        }

        let pipe_indent = indent + 2;
        let rhs_break_indent = indent + 6;

        let mut text = String::from("module");
        for (k, (name, binding)) in names.iter().zip(bindings.iter()).enumerate() {
            let b_sc = sc.and_then(|s| s.child(k));
            let rhs_r = self.render(binding, b_sc, rhs_break_indent);
            text.push('\n');
            text.push_str(&Self::pad(pipe_indent));
            if rhs_r.inline {
                let line = format!("| {} = {}", name, rhs_r.text);
                text.push_str(&Self::annotate(&line, &rhs_r.annots, self.flags));
            } else {
                text.push_str(&format!(
                    "| {} =\n{}{}",
                    name,
                    Self::pad(rhs_break_indent),
                    rhs_r.text
                ));
            }
        }

        for _ in 0..n {
            self.stack.pop();
        }

        let result = Rendered::always_break(text);
        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    // -----------------------------------------------------------------------
    // Unit artifact — always-break.
    // -----------------------------------------------------------------------

    fn render_unit(
        &mut self,
        node: &Node,
        imports: &[Node],
        exports: &[Node],
        defs: &[Node],
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        let unit_name = sc.and_then(|s| s.unit_alias.as_deref()).unwrap_or("Unit");
        let def_map = def_map_by_hash(defs);
        let export_hashes: BTreeSet<String> = exports
            .iter()
            .filter_map(|entry| match entry {
                Node::Export { hash, .. } => Some(hash.clone()),
                _ => None,
            })
            .collect();

        let mut text = format!("unit {}", unit_name);

        text.push('\n');
        text.push_str(&Self::pad(indent));
        text.push_str("imports");
        if imports.is_empty() {
            text.push('\n');
            text.push_str(&Self::pad(indent + 2));
            text.push_str("<none>");
        } else {
            let mut ordered: Vec<&Node> = imports.iter().collect();
            ordered.sort_by_key(|entry| unit_entry_hash(entry));
            for entry in ordered {
                if let Node::Import { hash, sig } = entry {
                    let alias = self
                        .import_alias(hash)
                        .unwrap_or_else(|| synthetic_hash_name("import", hash));
                    text.push('\n');
                    text.push_str(&Self::pad(indent + 2));
                    text.push_str(&format!(
                        "{} : {} = {}",
                        alias,
                        format_signature(sig),
                        hash_text(hash, self.flags.hashes)
                    ));
                } else if let Node::HostImport {
                    capability,
                    operation,
                    sig,
                } = entry
                {
                    let hash = hash_hex(entry);
                    let alias = self
                        .import_alias(&hash)
                        .unwrap_or_else(|| synthetic_hash_name("host_import", &hash));
                    text.push('\n');
                    text.push_str(&Self::pad(indent + 2));
                    text.push_str(&format!(
                        "host {} : {} from capability {} operation {} = {}",
                        alias,
                        format_signature(sig),
                        render_string_inline(capability),
                        render_string_inline(operation),
                        hash_text(&hash, self.flags.hashes)
                    ));
                }
            }
        }

        text.push('\n');
        text.push_str(&Self::pad(indent));
        text.push_str("exports");
        if exports.is_empty() {
            text.push('\n');
            text.push_str(&Self::pad(indent + 2));
            text.push_str("<none>");
        } else {
            let mut ordered: Vec<&Node> = exports.iter().collect();
            ordered.sort_by_key(|entry| unit_entry_hash(entry));
            for entry in ordered {
                if let Node::Export { visibility, hash } = entry {
                    let alias = self
                        .export_alias(hash)
                        .or_else(|| self.definition_alias(hash))
                        .unwrap_or_else(|| synthetic_hash_name("export", hash));
                    let sig = def_map
                        .get(hash)
                        .map(|def| format_signature(def_sig(def)))
                        .unwrap_or_else(|| "<dangling>".to_string());
                    text.push('\n');
                    text.push_str(&Self::pad(indent + 2));
                    text.push_str(&format!(
                        "{} {} : {} = {}",
                        visibility,
                        alias,
                        sig,
                        hash_text(hash, self.flags.hashes)
                    ));
                }
            }
        }

        let private_defs: Vec<(&String, &Node)> = def_map
            .iter()
            .filter(|(hash, _)| !export_hashes.contains(*hash))
            .map(|(hash, def)| (hash, *def))
            .collect();
        text.push('\n');
        text.push_str(&Self::pad(indent));
        text.push_str("private");
        if private_defs.is_empty() {
            text.push('\n');
            text.push_str(&Self::pad(indent + 2));
            text.push_str("<none>");
        } else {
            for (hash, def) in private_defs {
                let alias = self
                    .definition_alias(hash)
                    .unwrap_or_else(|| synthetic_hash_name("def", hash));
                text.push('\n');
                text.push_str(&Self::pad(indent + 2));
                text.push_str(&format!(
                    "{} : {} = {}",
                    alias,
                    format_signature(def_sig(def)),
                    hash_text(hash, self.flags.hashes)
                ));
            }
        }

        text.push('\n');
        text.push_str(&Self::pad(indent));
        text.push_str("definitions");
        let mut ordered_defs: Vec<(&String, &Node)> =
            def_map.iter().map(|(hash, def)| (hash, *def)).collect();
        ordered_defs.sort_by(|a, b| a.0.cmp(b.0));
        for (hash, def) in ordered_defs {
            let alias = self
                .definition_alias(hash)
                .or_else(|| self.export_alias(hash))
                .unwrap_or_else(|| synthetic_hash_name("def", hash));
            if let Node::Def { body, .. } = def {
                let body_r = self.render(body, sc, indent + 4);
                text.push('\n');
                text.push_str(&Self::pad(indent + 2));
                if body_r.inline {
                    text.push_str(&format!("{} = {}", alias, body_r.text));
                } else {
                    text.push_str(&format!(
                        "{} =\n{}{}",
                        alias,
                        Self::pad(indent + 4),
                        body_r.text
                    ));
                }
            }
        }

        let result = Rendered::always_break(text);
        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    fn render_imports(
        &mut self,
        entries: &[Node],
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        let mut text = String::from("imports");
        let mut ordered: Vec<&Node> = entries.iter().collect();
        ordered.sort_by_key(|entry| unit_entry_hash(entry));
        for entry in ordered {
            let r = self.render(entry, sc, indent + 2);
            text.push('\n');
            text.push_str(&Self::pad(indent + 2));
            text.push_str(&r.text);
        }
        Rendered::always_break(text)
    }

    fn render_exports(
        &mut self,
        entries: &[Node],
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        let mut text = String::from("exports");
        let mut ordered: Vec<&Node> = entries.iter().collect();
        ordered.sort_by_key(|entry| unit_entry_hash(entry));
        for entry in ordered {
            let r = self.render(entry, sc, indent + 2);
            text.push('\n');
            text.push_str(&Self::pad(indent + 2));
            text.push_str(&r.text);
        }
        Rendered::always_break(text)
    }

    fn render_defs(&mut self, defs: &[Node], sc: Option<&SidecarNode>, indent: usize) -> Rendered {
        let def_map = def_map_by_hash(defs);
        let mut text = String::from("defs");
        let mut states: Vec<&Node> = defs
            .iter()
            .filter(|def| matches!(def, Node::State { .. }))
            .collect();
        states.sort_by_key(|state| hash_hex(state));
        for state in states {
            let r = self.render(state, sc, indent + 2);
            text.push('\n');
            text.push_str(&Self::pad(indent + 2));
            text.push_str(&r.text);
        }
        for (_, def) in def_map {
            let r = self.render(def, sc, indent + 2);
            text.push('\n');
            text.push_str(&Self::pad(indent + 2));
            text.push_str(&r.text);
        }
        Rendered::always_break(text)
    }

    fn render_def(
        &mut self,
        sig: &Node,
        body: &Node,
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        let body_r = self.render(body, sc, indent + 2);
        if body_r.inline {
            Rendered::leaf(format!("def : {} = {}", format_signature(sig), body_r.text))
        } else {
            Rendered::always_break(format!(
                "def : {} =\n{}{}",
                format_signature(sig),
                Self::pad(indent + 2),
                body_r.text
            ))
        }
    }

    // -----------------------------------------------------------------------
    // If — always-break; handles chained else-if.
    // -----------------------------------------------------------------------

    fn render_if(
        &mut self,
        node: &Node,
        cond: &Node,
        then: &Node,
        else_: &Node,
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        let cond_sc = sc.and_then(|s| s.child(0));
        let then_sc = sc.and_then(|s| s.child(1));
        let else_sc = sc.and_then(|s| s.child(2));

        let cond_r = self.render(cond, cond_sc, indent + 2);
        let then_r = self.render(then, then_sc, indent + 2);

        let mut text = String::new();

        if cond_r.inline {
            let line = format!("if {}", cond_r.text);
            text.push_str(&Self::annotate(&line, &cond_r.annots, self.flags));
        } else {
            text.push_str(&format!("if\n{}{}", Self::pad(indent + 2), cond_r.text));
        }
        text.push('\n');
        text.push_str(&Self::pad(indent));

        if then_r.inline {
            let line = format!("then {}", then_r.text);
            text.push_str(&Self::annotate(&line, &then_r.annots, self.flags));
        } else {
            text.push_str(&format!("then\n{}{}", Self::pad(indent + 2), then_r.text));
        }
        text.push('\n');
        text.push_str(&Self::pad(indent));

        // Chained else-if stays at the same indent.
        if matches!(else_, Node::If { .. }) {
            let else_r = self.render(else_, else_sc, indent);
            text.push_str(&format!("else {}", else_r.text));
        } else {
            let else_r = self.render(else_, else_sc, indent + 2);
            if else_r.inline {
                let line = format!("else {}", else_r.text);
                text.push_str(&Self::annotate(&line, &else_r.annots, self.flags));
            } else {
                text.push_str(&format!("else\n{}{}", Self::pad(indent + 2), else_r.text));
            }
        }

        let result = Rendered::always_break(text);
        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    // -----------------------------------------------------------------------
    // Match + Arm — always-break.
    // -----------------------------------------------------------------------

    fn render_match(
        &mut self,
        node: &Node,
        scrutinee: &Node,
        arms: &[Node],
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        let scr_sc = sc.and_then(|s| s.child(0));
        let scr_r = self.render(scrutinee, scr_sc, indent + 2);

        let mut text = if scr_r.inline {
            let line = format!("match {}", scr_r.text);
            Self::annotate(&line, &scr_r.annots, self.flags)
        } else {
            format!("match\n{}{}", Self::pad(indent + 2), scr_r.text)
        };

        for (i, arm) in arms.iter().enumerate() {
            let arm_sc = sc.and_then(|s| s.child(i + 1));
            let arm_r = self.render(arm, arm_sc, indent);
            text.push('\n');
            text.push_str(&Self::pad(indent));
            text.push_str(&arm_r.text);
        }

        let result = Rendered::always_break(text);
        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    fn render_arm(
        &mut self,
        node: &Node,
        pattern: &Node,
        body: &Node,
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        let pat_sc = sc.and_then(|s| s.child(0));
        let body_sc = sc.and_then(|s| s.child(1));

        self.pat_var_counter = 0;
        let (pat_text, pat_vars) = self.render_pattern(pattern, pat_sc);

        // Push pat-vars in textual order; last pushed = var 0 (DeBruijn rule).
        let save_len = self.stack.len();
        for name in &pat_vars {
            self.stack.push(name.clone());
        }

        let body_r = self.render(body, body_sc, indent + 4);
        self.stack.truncate(save_len);

        let result = if body_r.inline {
            let line = format!("| {} => {}", pat_text, body_r.text);
            let line = Self::annotate(&line, &body_r.annots, self.flags);
            Rendered {
                text: line,
                inline: true,
                annots: Vec::new(),
            }
        } else {
            let text = format!(
                "| {} =>\n{}{}",
                pat_text,
                Self::pad(indent + 4),
                body_r.text
            );
            Rendered::always_break(text)
        };

        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    /// Render a pattern, returning (text, pat_var_names_in_textual_order).
    fn render_pattern(&mut self, node: &Node, sc: Option<&SidecarNode>) -> (String, Vec<String>) {
        match node {
            Node::PatWild => ("_".to_string(), Vec::new()),
            Node::PatVar => {
                let name = sc
                    .and_then(|s| s.binder.as_deref())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        let n = format!("p{}", self.pat_var_counter);
                        n
                    });
                self.pat_var_counter += 1;
                (name.clone(), vec![name])
            }
            Node::PatCtor { name, sub_patterns } => {
                self.render_pattern_node(node, name, sub_patterns, sc)
            }
            _ => ("_".to_string(), Vec::new()),
        }
    }

    fn render_pattern_node(
        &mut self,
        _node: &Node,
        name: &str,
        sub_patterns: &[Node],
        sc: Option<&SidecarNode>,
    ) -> (String, Vec<String>) {
        let mut text = name.to_string();
        let mut all_vars = Vec::new();
        for (k, sp) in sub_patterns.iter().enumerate() {
            let sp_sc = sc.and_then(|s| s.child(k + 1));
            let (sp_text, sp_vars) = self.render_pattern(sp, sp_sc);
            text.push(' ');
            // Parenthesize non-nullary nested pat-ctors.
            if matches!(sp, Node::PatCtor { sub_patterns: sps, .. } if !sps.is_empty()) {
                text.push('(');
                text.push_str(&sp_text);
                text.push(')');
            } else {
                text.push_str(&sp_text);
            }
            all_vars.extend(sp_vars);
        }
        (text, all_vars)
    }

    // -----------------------------------------------------------------------
    // Record — inline iff empty or single inline field.
    // -----------------------------------------------------------------------

    fn render_record(
        &mut self,
        node: &Node,
        fields: &[(String, Node)],
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        if fields.is_empty() {
            let r = Rendered::leaf("{}");
            return if self.flags.hashes {
                self.badge(node, r)
            } else {
                r
            };
        }

        let n = fields.len();
        let render_order: Vec<usize> = sc
            .and_then(|s| s.field_order.as_ref())
            .cloned()
            .unwrap_or_else(|| (0..n).collect());

        let mut rendered_fields: Vec<(String, Rendered)> = Vec::new();
        for &ci in &render_order {
            if ci < n {
                let (key, val) = &fields[ci];
                let val_sc = sc.and_then(|s| s.child(2 * ci + 1));
                let val_r = self.render(val, val_sc, indent + 2);
                rendered_fields.push((key.clone(), val_r));
            }
        }

        // Single inline field → inline form.
        if rendered_fields.len() == 1 && rendered_fields[0].1.inline {
            let (key, val_r) = &rendered_fields[0];
            let text = format!("{{ {}: {} }}", key, val_r.text);
            let r = Rendered {
                text,
                inline: true,
                annots: val_r.annots.clone(),
            };
            return if self.flags.hashes {
                self.badge(node, r)
            } else {
                r
            };
        }

        // Multi-field or non-inline: one-field-per-line with trailing comma.
        // L1 annotation appends after the comma so it doesn't swallow it.
        let mut text = String::from("{");
        for (key, val_r) in &rendered_fields {
            text.push('\n');
            text.push_str(&Self::pad(indent + 2));
            let field_line = format!("{}: {},", key, val_r.text);
            if val_r.inline {
                text.push_str(&Self::annotate(&field_line, &val_r.annots, self.flags));
            } else {
                text.push_str(&field_line);
            }
        }
        text.push('\n');
        text.push_str(&Self::pad(indent));
        text.push('}');

        let result = Rendered::always_break(text);
        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    // -----------------------------------------------------------------------
    // Proj — flatten chains; inline iff record sub-expression is inline.
    // -----------------------------------------------------------------------

    fn render_proj(
        &mut self,
        node: &Node,
        record: &Node,
        field: &str,
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        // Collect the projection chain from outermost to innermost.
        let mut fields: Vec<&str> = vec![field];
        let mut cur: &Node = record;
        let mut cur_sc = sc.and_then(|s| s.child(0));

        while let Node::Proj {
            record: inner_rec,
            field: inner_field,
        } = cur
        {
            fields.push(inner_field.as_str());
            let next_sc = cur_sc.and_then(|s| s.child(0));
            cur = inner_rec;
            cur_sc = next_sc;
        }
        fields.reverse(); // now innermost field first → a.b.c order

        let rec_r = self.render(cur, cur_sc, indent);
        let fields_str = fields.join(".");

        let result = if rec_r.inline {
            Rendered {
                text: format!("{}.{}", rec_r.text, fields_str),
                inline: true,
                annots: rec_r.annots,
            }
        } else {
            Rendered {
                text: format!("{}.{}", rec_r.text, fields_str),
                inline: false,
                annots: Vec::new(),
            }
        };

        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    // -----------------------------------------------------------------------
    // Ctor — inline iff all args are inline.
    // -----------------------------------------------------------------------

    fn render_ctor(
        &mut self,
        node: &Node,
        name: &str,
        args: &[Node],
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        if args.is_empty() {
            let r = Rendered::leaf(name.to_string());
            return if self.flags.hashes {
                self.badge(node, r)
            } else {
                r
            };
        }

        let mut arg_rs: Vec<Rendered> = Vec::new();
        for (k, arg) in args.iter().enumerate() {
            // Canonical children of (ctor name arg₀ …) are [name-sym, arg₀, …];
            // skip child(0) (the name-sym slot) and start args at child(1).
            let arg_sc = sc.and_then(|s| s.child(k + 1));
            let ar = self.render_as_arg(arg, arg_sc, indent + 2);
            arg_rs.push(ar);
        }

        let all_inline = arg_rs.iter().all(|r| r.inline);
        let result = if all_inline {
            let args_text = arg_rs
                .iter()
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let text = format!("{} {}", name, args_text);
            // Propagate annots; parent line-builder applies the trailing comment.
            let annots: Vec<_> = arg_rs
                .iter()
                .flat_map(|r| r.annots.iter().cloned())
                .collect();
            Rendered {
                text,
                inline: true,
                annots,
            }
        } else {
            let mut text = name.to_string();
            for ar in &arg_rs {
                text.push('\n');
                text.push_str(&Self::pad(indent + 2));
                text.push_str(&ar.text);
            }
            Rendered::always_break(text)
        };

        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }

    // -----------------------------------------------------------------------
    // Ann — inline iff both sub-expressions are inline.
    // -----------------------------------------------------------------------

    fn render_ann(
        &mut self,
        node: &Node,
        expr: &Node,
        type_: &Node,
        sc: Option<&SidecarNode>,
        indent: usize,
    ) -> Rendered {
        let expr_sc = sc.and_then(|s| s.child(0));
        let expr_r = self.render(expr, expr_sc, indent + 1);
        let type_r = Rendered::leaf(format_type_node_nice(type_));

        let result = if expr_r.inline && type_r.inline {
            let text = format!("({} : {})", expr_r.text, type_r.text);
            let annots: Vec<_> = expr_r.annots.into_iter().chain(type_r.annots).collect();
            Rendered {
                text,
                inline: true,
                annots,
            }
        } else {
            let text = format!("({} : {})", expr_r.text, type_r.text);
            Rendered::always_break(text)
        };

        if self.flags.hashes {
            self.badge(node, result)
        } else {
            result
        }
    }
}

fn hash_text(hash: &str, full: bool) -> String {
    if full || hash.len() <= 8 {
        format!("blake3:{}", hash)
    } else {
        format!("blake3:{}...", &hash[..8])
    }
}

fn synthetic_hash_name(prefix: &str, hash: &str) -> String {
    let short = if hash.len() >= 8 { &hash[..8] } else { hash };
    format!("{}_{}", prefix, short)
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

fn render_string_inline(value: &str) -> String {
    let mut out = String::from('"');
    for ch in value.chars() {
        let cp = ch as u32;
        match cp {
            0x22 => out.push_str("\\\""),
            0x5C => out.push_str("\\\\"),
            0x09 => out.push_str("\\t"),
            0x0A => out.push_str("\\n"),
            0x0D => out.push_str("\\r"),
            0x20..=0x7E => out.push(ch),
            _ => out.push_str(&format!("\\u{{{:x}}}", cp)),
        }
    }
    out.push('"');
    out
}

fn def_map_by_hash(defs: &[Node]) -> BTreeMap<String, &Node> {
    defs.iter()
        .filter_map(|def| match def {
            Node::Def { .. } => {
                let hash = hash_node(def);
                let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
                Some((hex, def))
            }
            _ => None,
        })
        .collect()
}

fn def_sig(def: &Node) -> &Node {
    match def {
        Node::Def { sig, .. } => sig,
        _ => def,
    }
}

fn format_signature(sig: &Node) -> String {
    match sig {
        Node::Sig { type_, eval_eff } => format_signature_parts(type_, eval_eff),
        other => String::from_utf8_lossy(&tacit_canonical::emit(other)).into_owned(),
    }
}

fn format_signature_parts(type_: &Node, eval_eff: &Node) -> String {
    let type_text = format_type_node_nice(type_);
    match eval_eff {
        Node::EffSet { atoms } if atoms.is_empty() => type_text,
        Node::EffSet { atoms } => format!("{} eval {{{}}}", type_text, atoms.join(", ")),
        other => format!(
            "{} eval {}",
            type_text,
            String::from_utf8_lossy(&tacit_canonical::emit(other))
        ),
    }
}

// ---------------------------------------------------------------------------
// Phase 2 type/effect verbose rendering helpers (--types / --effects).
// ---------------------------------------------------------------------------

/// Render a type-level AST node as a human-readable string.
/// Used when `InspectFlags::types` is set.
fn format_type_node_nice(node: &Node) -> String {
    use tacit_canonical::emit::emit;
    match node {
        Node::Sym { name } => name.clone(),
        Node::TyVar { index } => format!("\u{03b1}{}", index),
        Node::Record { fields } => {
            if fields.is_empty() {
                "{}".to_string()
            } else {
                let rendered = fields
                    .iter()
                    .map(|(name, value)| format!("{}: {}", name, format_type_node_nice(value)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{}}}", rendered)
            }
        }
        Node::App { fn_, arg } => {
            format!(
                "{} {}",
                format_type_node_nice(fn_),
                format_type_node_nice(arg)
            )
        }
        Node::FnTy { arg, ret, eff } => {
            let arg_s = format_type_node_nice(arg);
            let ret_s = format_type_node_nice(ret);
            let eff_s = format_eff_node_nice(eff, true);
            let needs_parens = matches!(arg.as_ref(), Node::FnTy { .. } | Node::Forall { .. });
            if needs_parens {
                format!("({}) -> {}{}", arg_s, ret_s, eff_s)
            } else {
                format!("{} -> {}{}", arg_s, ret_s, eff_s)
            }
        }
        Node::Forall {
            ty_count,
            eff_count,
            body,
        } => format_forall_nice(*ty_count, *eff_count, body),
        Node::EffSet { atoms } => {
            if atoms.is_empty() {
                "{}".to_string()
            } else {
                format!("{{{}}}", atoms.join(", "))
            }
        }
        Node::EffVar { index } => format!("\u{03b5}{}", index),
        other => String::from_utf8_lossy(&emit(other)).into_owned(),
    }
}

/// Render an effect node as a `/ <eff>` suffix string.
/// Returns an empty string for a pure (empty) effect set when `verbose` is true.
fn format_eff_node_nice(node: &Node, verbose: bool) -> String {
    use tacit_canonical::emit::emit;
    if !verbose {
        return String::from_utf8_lossy(&emit(node)).into_owned();
    }
    match node {
        Node::EffSet { atoms } if atoms.is_empty() => String::new(),
        Node::EffSet { atoms } => format!(" / {{{}}}", atoms.join(", ")),
        Node::EffVar { index } => format!(" / \u{03b5}{}", index),
        other => format!(" / {}", String::from_utf8_lossy(&emit(other))),
    }
}

fn format_forall_nice(ty_count: u32, eff_count: u32, body: &Node) -> String {
    let mut vars: Vec<String> = (0..ty_count).map(|i| format!("\u{03b1}{}", i)).collect();
    vars.extend((0..eff_count).map(|i| format!("\u{03b5}{}", i)));
    let body_s = format_type_node_nice(body);
    format!("\u{2200}[{}]. {}", vars.join(", "), body_s)
}

fn combinator_labels(name: &str, arity: usize) -> Option<&'static [&'static str]> {
    match (name, arity) {
        ("map", 4) => Some(&["input", "count", "callback", "output"]),
        ("fold", 4) => Some(&["input", "count", "initial", "callback"]),
        ("for-each", 3) => Some(&["input", "count", "callback"]),
        _ => None,
    }
}

fn collect_free_outer_indices(node: &Node, depth: u64, out: &mut BTreeSet<usize>) {
    match node {
        Node::Var { index } => {
            if *index >= depth {
                out.insert((*index - depth) as usize);
            }
        }
        Node::Lam { body } => collect_free_outer_indices(body, depth + 1, out),
        Node::Let { rhs, body } => {
            collect_free_outer_indices(rhs, depth, out);
            collect_free_outer_indices(body, depth + 1, out);
        }
        Node::Rec { bindings, body } => {
            let inner = depth + bindings.len() as u64;
            for binding in bindings {
                collect_free_outer_indices(binding, inner, out);
            }
            collect_free_outer_indices(body, inner, out);
        }
        Node::Module { bindings } => {
            let inner = depth + bindings.len() as u64;
            for binding in bindings {
                collect_free_outer_indices(binding, inner, out);
            }
        }
        Node::Unit {
            imports: _,
            exports: _,
            defs,
        } => {
            for def in defs {
                collect_free_outer_indices(def, depth, out);
            }
        }
        Node::Defs { defs } => {
            for def in defs {
                collect_free_outer_indices(def, depth, out);
            }
        }
        Node::Def { body, .. } => collect_free_outer_indices(body, depth, out),
        Node::App { fn_, arg } => {
            collect_free_outer_indices(fn_, depth, out);
            collect_free_outer_indices(arg, depth, out);
        }
        Node::If { cond, then, else_ } => {
            collect_free_outer_indices(cond, depth, out);
            collect_free_outer_indices(then, depth, out);
            collect_free_outer_indices(else_, depth, out);
        }
        Node::Match { scrutinee, arms } => {
            collect_free_outer_indices(scrutinee, depth, out);
            for arm in arms {
                collect_free_outer_indices(arm, depth, out);
            }
        }
        Node::Arm { pattern, body } => {
            collect_free_outer_indices(body, depth + count_pat_vars(pattern), out);
        }
        Node::Record { fields } => {
            for (_, value) in fields {
                collect_free_outer_indices(value, depth, out);
            }
        }
        Node::Proj { record, .. } => collect_free_outer_indices(record, depth, out),
        Node::Ctor { args, .. } => {
            for arg in args {
                collect_free_outer_indices(arg, depth, out);
            }
        }
        Node::Ann { expr, .. } => collect_free_outer_indices(expr, depth, out),
        Node::PatCtor { sub_patterns, .. } => {
            for pattern in sub_patterns {
                collect_free_outer_indices(pattern, depth, out);
            }
        }
        Node::Int { .. }
        | Node::Str { .. }
        | Node::Sym { .. }
        | Node::Hole { .. }
        | Node::Imports { .. }
        | Node::Import { .. }
        | Node::HostImport { .. }
        | Node::Exports { .. }
        | Node::Export { .. }
        | Node::State { .. }
        | Node::Sig { .. }
        | Node::Ref { .. }
        | Node::PatWild
        | Node::PatVar
        | Node::PatInt { .. }
        | Node::FnTy { .. }
        | Node::TyVar { .. }
        | Node::Forall { .. }
        | Node::EffSet { .. }
        | Node::EffVar { .. } => {}
    }
}

fn count_pat_vars(node: &Node) -> u64 {
    match node {
        Node::PatVar => 1,
        Node::PatCtor { sub_patterns, .. } => sub_patterns.iter().map(count_pat_vars).sum(),
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Sidecar comment rendering (inspection-view.md § 5).
// ---------------------------------------------------------------------------

fn prepend_comment(r: Rendered, comment: &str, indent: usize) -> Rendered {
    let needs_multiline = comment.len() > 80 || comment.contains('\n');
    let prefix = if needs_multiline {
        comment
            .lines()
            .map(|line| format!("# {}", line))
            .collect::<Vec<_>>()
            .join(&format!("\n{}", " ".repeat(indent)))
    } else {
        format!("# {}", comment)
    };
    Rendered {
        text: format!("{}\n{}{}", prefix, " ".repeat(indent), r.text),
        inline: false,
        annots: Vec::new(),
    }
}
