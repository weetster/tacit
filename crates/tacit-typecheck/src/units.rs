//! Unit artifact checking.
//!
//! This layer is intentionally separate from `infer_module`: single-program
//! inputs still infer directly, while canonical `unit` artifacts get an
//! import/export resolution pass before ordinary expression inference.

use std::collections::{BTreeMap, BTreeSet};

use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node};
use tacit_views::sidecar::SidecarNode;

use crate::error::Diagnostic;
use crate::infer::{infer, with_state_context};
use crate::ty::{unify, EffSet, FnEff, Subst, Ty};
use crate::type_from_node::{eff_from_node, type_from_node};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionVisibility {
    Public,
    Package,
    Private,
}

impl DefinitionVisibility {
    fn as_str(self) -> &'static str {
        match self {
            DefinitionVisibility::Public => "public",
            DefinitionVisibility::Package => "package",
            DefinitionVisibility::Private => "private",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProvidedDefinition {
    pub def: Node,
    pub visibility: DefinitionVisibility,
    pub same_package: bool,
}

impl ProvidedDefinition {
    pub fn new(def: Node, visibility: DefinitionVisibility, same_package: bool) -> Self {
        Self {
            def,
            visibility,
            same_package,
        }
    }
}

pub type DefinitionEnv = BTreeMap<String, ProvidedDefinition>;

#[derive(Debug)]
pub struct CheckedUnit {
    pub definition_types: BTreeMap<String, Ty>,
    pub definition_effects: BTreeMap<String, EffSet>,
}

#[derive(Debug, Default)]
struct UnitAliases {
    import_aliases: BTreeMap<String, String>,
    definition_aliases: BTreeMap<String, String>,
    export_aliases: BTreeMap<String, String>,
}

impl UnitAliases {
    fn from_sidecar(sidecar: Option<&SidecarNode>) -> Self {
        Self {
            import_aliases: sidecar
                .and_then(|s| s.import_aliases.clone())
                .unwrap_or_default(),
            definition_aliases: sidecar
                .and_then(|s| s.definition_aliases.clone())
                .unwrap_or_default(),
            export_aliases: sidecar
                .and_then(|s| s.export_aliases.clone())
                .unwrap_or_default(),
        }
    }

    fn import_alias(&self, hash: &str) -> Option<&str> {
        self.alias_from_map(&self.import_aliases, hash)
    }

    fn definition_alias(&self, hash: &str) -> Option<&str> {
        self.alias_from_map(&self.definition_aliases, hash)
    }

    fn export_alias(&self, hash: &str) -> Option<&str> {
        self.alias_from_map(&self.export_aliases, hash)
    }

    fn boundary_alias(&self, hash: &str) -> Option<&str> {
        self.import_alias(hash)
            .or_else(|| self.definition_alias(hash))
            .or_else(|| self.export_alias(hash))
    }

    fn exported_definition_alias(&self, hash: &str) -> Option<&str> {
        self.export_alias(hash)
            .or_else(|| self.definition_alias(hash))
    }

    fn alias_from_map<'a>(
        &'a self,
        map: &'a BTreeMap<String, String>,
        hash: &str,
    ) -> Option<&'a str> {
        let alias = map.get(hash)?;
        self.alias_is_unambiguous(hash, alias)
            .then_some(alias.as_str())
    }

    fn alias_is_unambiguous(&self, hash: &str, alias: &str) -> bool {
        for map in [
            &self.import_aliases,
            &self.definition_aliases,
            &self.export_aliases,
        ] {
            for (candidate_hash, candidate_alias) in map {
                if candidate_alias == alias && candidate_hash != hash {
                    return false;
                }
            }
        }
        true
    }

    fn display_hash(&self, hash: &str) -> String {
        self.boundary_alias(hash)
            .map(|alias| format!("{} (blake3:{})", alias, hash))
            .unwrap_or_else(|| format!("blake3:{}", hash))
    }
}

pub fn check_units_in_memory(units: &[Node]) -> Result<Vec<CheckedUnit>, Vec<Diagnostic>> {
    let mut diags = Vec::new();
    let mut env = DefinitionEnv::new();
    let aliases = UnitAliases::default();

    for (unit_index, unit) in units.iter().enumerate() {
        let Some(parts) = unit_artifact_parts(unit) else {
            diags.push(Diagnostic::invalid_unit_artifact(&[unit_index]));
            continue;
        };
        let local_defs = local_def_map(parts.defs);
        for export in parts.exports {
            let Node::Export { visibility, hash } = export else {
                continue;
            };
            let Some(def) = local_defs.get(hash) else {
                continue;
            };
            let visibility = parse_visibility(visibility).unwrap_or(DefinitionVisibility::Public);
            env.insert(
                hash.clone(),
                ProvidedDefinition::new((*def).clone(), visibility, true),
            );
        }
    }

    let mut typed = Vec::new();
    for (unit_index, unit) in units.iter().enumerate() {
        match check_unit_with_path(unit, &env, &[unit_index], &aliases) {
            Ok(t) => typed.push(t),
            Err(mut errors) => diags.append(&mut errors),
        }
    }

    if diags.is_empty() {
        Ok(typed)
    } else {
        Err(diags)
    }
}

pub fn check_unit(unit: &Node, providers: &DefinitionEnv) -> Result<CheckedUnit, Vec<Diagnostic>> {
    let aliases = UnitAliases::default();
    check_unit_with_path(unit, providers, &[], &aliases)
}

pub fn check_unit_with_sidecar(
    unit: &Node,
    providers: &DefinitionEnv,
    sidecar: Option<&SidecarNode>,
) -> Result<CheckedUnit, Vec<Diagnostic>> {
    let aliases = UnitAliases::from_sidecar(sidecar);
    check_unit_with_path(unit, providers, &[], &aliases)
}

fn check_unit_with_path(
    unit: &Node,
    providers: &DefinitionEnv,
    path: &[usize],
    aliases: &UnitAliases,
) -> Result<CheckedUnit, Vec<Diagnostic>> {
    let Some(parts) = unit_artifact_parts(unit) else {
        return Err(vec![Diagnostic::invalid_unit_artifact(path)]);
    };

    let mut diags = Vec::new();
    let local_defs = local_def_map(parts.defs);
    let state_fields = validate_state_entries(parts.defs, path, &mut diags);

    let mut seen_imports = BTreeSet::new();
    let mut seen_host_imports = BTreeSet::new();
    let mut import_sigs: BTreeMap<String, Node> = BTreeMap::new();
    for (i, import) in parts.imports.iter().enumerate() {
        let imp_path = child_path(path, &[0, i]);
        match import {
            Node::Import { hash, sig } => {
                if !seen_imports.insert(hash.clone()) {
                    diags.push(Diagnostic::duplicate_import(
                        &imp_path,
                        hash,
                        aliases.import_alias(hash),
                    ));
                }
                import_sigs.insert(hash.clone(), sig.as_ref().clone());

                let Some(provider) = providers.get(hash) else {
                    diags.push(Diagnostic::missing_import(
                        &imp_path,
                        hash,
                        aliases.import_alias(hash),
                    ));
                    continue;
                };

                let actual_hash = hex_hash(&provider.def);
                if actual_hash != *hash {
                    diags.push(Diagnostic::hash_mismatch(
                        &imp_path,
                        hash,
                        &actual_hash,
                        aliases.import_alias(hash),
                    ));
                }

                match provider.visibility {
                    DefinitionVisibility::Public => {}
                    DefinitionVisibility::Package if provider.same_package => {}
                    other => diags.push(Diagnostic::visibility_violation(
                        &imp_path,
                        hash,
                        other.as_str(),
                        aliases.import_alias(hash),
                    )),
                }

                let provider_sig = definition_sig(&provider.def);
                let expected = canonical_text(sig);
                let actual = canonical_text(provider_sig);
                if expected != actual {
                    let subject = format!(
                        "import {}",
                        aliases
                            .import_alias(hash)
                            .map(|alias| format!("{} (blake3:{})", alias, hash))
                            .unwrap_or_else(|| format!("blake3:{}", hash))
                    );
                    diags.push(Diagnostic::signature_mismatch(
                        &imp_path, &subject, &expected, &actual,
                    ));
                }
            }
            Node::HostImport {
                capability,
                operation,
                sig,
            } => {
                let hash = hex_hash(import);
                if !seen_host_imports.insert(hash.clone()) {
                    diags.push(Diagnostic::duplicate_host_import(
                        &imp_path,
                        &hash,
                        aliases.import_alias(&hash),
                    ));
                }
                import_sigs.insert(hash.clone(), sig.as_ref().clone());

                let mut subst = Subst::default();
                let mut local_diags = Vec::new();
                let ty = signature_type(sig, &mut subst, &mut local_diags);
                let _eval_eff = signature_eval_eff(sig, &mut subst, &mut local_diags);
                let flattened = flatten_host_import_effects(&subst.apply(&ty), &subst);
                match flattened {
                    Some(effects) if effects.atoms.contains(&crate::ty::EffAtom::IO) => {}
                    _ => diags.push(Diagnostic::host_import_signature_mismatch(
                        &imp_path,
                        &hash,
                        capability,
                        operation,
                        "host import function effects must include IO",
                    )),
                }
                diags.extend(local_diags.into_iter().filter(|d| d.severity == "error"));
            }
            _ => {}
        }
    }

    let mut seen_exports = BTreeSet::new();
    for (i, export) in parts.exports.iter().enumerate() {
        let Node::Export { hash, .. } = export else {
            continue;
        };
        let exp_path = child_path(path, &[1, i]);
        if !seen_exports.insert(hash.clone()) {
            diags.push(Diagnostic::duplicate_export(
                &exp_path,
                hash,
                aliases.exported_definition_alias(hash),
            ));
        }
        if !local_defs.contains_key(hash) {
            diags.push(Diagnostic::dangling_export(
                &exp_path,
                hash,
                aliases.exported_definition_alias(hash),
            ));
        }
    }

    let mut graph_defs: BTreeMap<String, &Node> = local_defs.clone();
    for (hash, provider) in providers {
        graph_defs.insert(hash.clone(), &provider.def);
    }
    detect_cycles(&graph_defs, path, aliases, &mut diags);

    let mut sig_env = BTreeMap::new();
    for (hash, def) in &local_defs {
        sig_env.insert(hash.clone(), definition_sig(def).clone());
    }
    for (hash, sig) in import_sigs {
        sig_env.insert(hash, sig);
    }

    let mut definition_types = BTreeMap::new();
    let mut definition_effects = BTreeMap::new();

    for (def_index, def) in parts.defs.iter().enumerate() {
        let hash = hex_hash(def);
        let def_path = child_path(path, &[2, def_index]);
        let Node::Def { sig, body } = def else {
            continue;
        };

        let mut missing_refs = Vec::new();
        collect_missing_refs(body, &sig_env, &mut missing_refs);
        for missing in missing_refs {
            diags.push(Diagnostic::missing_import(
                &def_path,
                &missing,
                aliases.boundary_alias(&missing),
            ));
        }

        let ref_hashes: Vec<String> = sig_env.keys().cloned().collect();
        let ref_indices: BTreeMap<String, usize> = ref_hashes
            .iter()
            .enumerate()
            .map(|(i, hash)| (hash.clone(), i))
            .collect();

        let mut subst = Subst::default();
        let mut local_diags = Vec::new();
        let mut ref_ctx = Vec::new();
        for ref_hash in &ref_hashes {
            let ref_sig = sig_env.get(ref_hash).expect("key came from sig_env");
            ref_ctx.push(signature_type(ref_sig, &mut subst, &mut local_diags));
        }

        let declared_ty = signature_type(sig, &mut subst, &mut local_diags);
        let declared_eval_eff = signature_eval_eff(sig, &mut subst, &mut local_diags);
        let rewritten_body = rewrite_refs(body, &ref_indices, 0);
        let (body_ty, body_eff) = with_state_context(state_fields.clone(), || {
            infer(
                &ref_ctx,
                &rewritten_body,
                &mut subst,
                &def_path,
                &mut local_diags,
            )
        });

        if !unify(&declared_ty, &body_ty, &mut subst) {
            let expected = subst.apply(&declared_ty);
            let actual = subst.apply(&body_ty);
            if !expected.is_unknown() && !actual.is_unknown() {
                let subject = format!("definition body {}", aliases.display_hash(&hash));
                diags.push(Diagnostic::signature_mismatch(
                    &def_path,
                    &subject,
                    &expected.to_string(),
                    &actual.to_string(),
                ));
            }
        }

        let body_eval_eff = subst.resolve_eff(&body_eff);
        if !body_eval_eff.is_subset_of(&declared_eval_eff) {
            let subject = format!("definition effects {}", aliases.display_hash(&hash));
            diags.push(Diagnostic::signature_mismatch(
                &def_path,
                &subject,
                &declared_eval_eff.to_string(),
                &body_eval_eff.to_string(),
            ));
        }

        diags.extend(local_diags.into_iter().filter(|d| d.severity == "error"));
        definition_types.insert(hash.clone(), subst.apply(&declared_ty));
        definition_effects.insert(hash, declared_eval_eff);
    }

    if diags.is_empty() {
        Ok(CheckedUnit {
            definition_types,
            definition_effects,
        })
    } else {
        Err(diags)
    }
}

struct UnitArtifactParts<'a> {
    imports: &'a [Node],
    exports: &'a [Node],
    defs: &'a [Node],
}

fn unit_artifact_parts(node: &Node) -> Option<UnitArtifactParts<'_>> {
    match node {
        Node::Unit {
            imports,
            exports,
            defs,
        } => {
            let valid_imports = imports
                .iter()
                .all(|entry| matches!(entry, Node::Import { .. } | Node::HostImport { .. }));
            let valid_exports = exports.iter().all(|entry| {
                matches!(
                    entry,
                    Node::Export {
                        visibility,
                        ..
                    } if visibility == "public" || visibility == "package"
                )
            });
            let valid_defs = !defs.is_empty()
                && defs
                    .iter()
                    .all(|entry| matches!(entry, Node::Def { .. } | Node::State { .. }));

            if valid_imports && valid_exports && valid_defs {
                Some(UnitArtifactParts {
                    imports,
                    exports,
                    defs,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn validate_state_entries(
    defs: &[Node],
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> Option<BTreeMap<String, Ty>> {
    let mut state_fields = None;
    let mut seen = false;
    for (i, def) in defs.iter().enumerate() {
        let Node::State { type_, .. } = def else {
            continue;
        };
        let state_path = child_path(path, &[2, i]);
        if seen {
            diags.push(Diagnostic::state_multiple(&state_path));
            continue;
        }
        seen = true;

        let mut subst = Subst::default();
        let mut local_diags = Vec::new();
        let ty = type_from_node(
            type_,
            &[],
            &[],
            &mut subst,
            &child_path(path, &[2, i, 1]),
            &mut local_diags,
        );
        let resolved = subst.apply(&ty);
        match resolved {
            Ty::Record(fields) => {
                for (field_index, (_name, field_ty)) in fields.iter().enumerate() {
                    validate_state_field_type(
                        field_ty,
                        &child_path(path, &[2, i, 1, field_index * 2 + 1]),
                        diags,
                    );
                }
                state_fields = Some(fields);
            }
            Ty::Unknown => {}
            other => diags.push(Diagnostic::state_field_shape_invalid(&state_path, &other)),
        }
        diags.extend(local_diags.into_iter().filter(|d| d.severity == "error"));
    }
    state_fields
}

fn validate_state_field_type(ty: &Ty, path: &[usize], diags: &mut Vec<Diagnostic>) {
    match ty {
        Ty::Int | Ty::IntLit | Ty::Bool | Ty::FixedInt(_) | Ty::Vec(_) | Ty::Unknown => {}
        Ty::Record(fields) => {
            for (i, field_ty) in fields.values().enumerate() {
                validate_state_field_type(field_ty, &child_path(path, &[i * 2 + 1]), diags);
            }
        }
        Ty::Str | Ty::Buf | Ty::I64Vec | Ty::Fn(_, _, _) | Ty::App(_, _) | Ty::Meta(_) => {
            diags.push(Diagnostic::state_field_shape_invalid(path, ty));
        }
    }
}

fn local_def_map(defs: &[Node]) -> BTreeMap<String, &Node> {
    defs.iter()
        .filter(|def| matches!(def, Node::Def { .. }))
        .map(|def| (hex_hash(def), def))
        .collect()
}

fn parse_visibility(s: &str) -> Option<DefinitionVisibility> {
    match s {
        "public" => Some(DefinitionVisibility::Public),
        "package" => Some(DefinitionVisibility::Package),
        _ => None,
    }
}

fn definition_sig(def: &Node) -> &Node {
    match def {
        Node::Def { sig, .. } => sig,
        other => other,
    }
}

fn definition_body(def: &Node) -> Option<&Node> {
    match def {
        Node::Def { body, .. } => Some(body),
        _ => None,
    }
}

fn signature_type(sig: &Node, subst: &mut Subst, diags: &mut Vec<Diagnostic>) -> Ty {
    match sig {
        Node::Sig { type_, .. } => type_from_node(type_, &[], &[], subst, &[], diags),
        other => type_from_node(other, &[], &[], subst, &[], diags),
    }
}

fn signature_eval_eff(sig: &Node, subst: &mut Subst, diags: &mut Vec<Diagnostic>) -> EffSet {
    match sig {
        Node::Sig { eval_eff, .. } => match eff_from_node(eval_eff, &[], subst, &[], diags) {
            FnEff::Concrete(set) => set,
            FnEff::Meta(_) => EffSet::empty(),
        },
        _ => EffSet::empty(),
    }
}

fn flatten_host_import_effects(ty: &Ty, subst: &Subst) -> Option<EffSet> {
    let mut cur = subst.apply(ty);
    let mut out = EffSet::empty();
    let mut saw_function = false;
    loop {
        match cur {
            Ty::Fn(_, ret, FnEff::Concrete(effects)) => {
                saw_function = true;
                out = out.join(&effects);
                cur = subst.apply(&ret);
            }
            Ty::Fn(_, _, FnEff::Meta(_)) => return None,
            _ => return saw_function.then_some(out),
        }
    }
}

fn canonical_text(node: &Node) -> String {
    String::from_utf8_lossy(&emit(node)).into_owned()
}

fn hex_hash(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn collect_missing_refs(node: &Node, sig_env: &BTreeMap<String, Node>, out: &mut Vec<String>) {
    match node {
        Node::Ref { hash } => {
            if !sig_env.contains_key(hash) {
                out.push(hash.clone());
            }
        }
        _ => for_each_expr_child(node, |child| collect_missing_refs(child, sig_env, out)),
    }
}

fn rewrite_refs(node: &Node, ref_indices: &BTreeMap<String, usize>, depth: u64) -> Node {
    match node {
        Node::Ref { hash } => ref_indices
            .get(hash)
            .map(|index| Node::Var {
                index: depth + *index as u64,
            })
            .unwrap_or_else(|| node.clone()),
        Node::Lam { body } => Node::Lam {
            body: Box::new(rewrite_refs(body, ref_indices, depth + 1)),
        },
        Node::Let { rhs, body } => Node::Let {
            rhs: Box::new(rewrite_refs(rhs, ref_indices, depth)),
            body: Box::new(rewrite_refs(body, ref_indices, depth + 1)),
        },
        Node::Rec { bindings, body } => {
            let inner = depth + bindings.len() as u64;
            Node::Rec {
                bindings: bindings
                    .iter()
                    .map(|binding| rewrite_refs(binding, ref_indices, inner))
                    .collect(),
                body: Box::new(rewrite_refs(body, ref_indices, inner)),
            }
        }
        Node::Module { bindings } => {
            let inner = depth + bindings.len() as u64;
            Node::Module {
                bindings: bindings
                    .iter()
                    .map(|binding| rewrite_refs(binding, ref_indices, inner))
                    .collect(),
            }
        }
        Node::App { fn_, arg } => Node::App {
            fn_: Box::new(rewrite_refs(fn_, ref_indices, depth)),
            arg: Box::new(rewrite_refs(arg, ref_indices, depth)),
        },
        Node::If { cond, then, else_ } => Node::If {
            cond: Box::new(rewrite_refs(cond, ref_indices, depth)),
            then: Box::new(rewrite_refs(then, ref_indices, depth)),
            else_: Box::new(rewrite_refs(else_, ref_indices, depth)),
        },
        Node::Match { scrutinee, arms } => Node::Match {
            scrutinee: Box::new(rewrite_refs(scrutinee, ref_indices, depth)),
            arms: arms
                .iter()
                .map(|arm| rewrite_refs(arm, ref_indices, depth))
                .collect(),
        },
        Node::Arm { pattern, body } => Node::Arm {
            pattern: pattern.clone(),
            body: Box::new(rewrite_refs(
                body,
                ref_indices,
                depth + count_pat_vars(pattern),
            )),
        },
        Node::Record { fields } => Node::Record {
            fields: fields
                .iter()
                .map(|(name, value)| (name.clone(), rewrite_refs(value, ref_indices, depth)))
                .collect(),
        },
        Node::Proj { record, field } => Node::Proj {
            record: Box::new(rewrite_refs(record, ref_indices, depth)),
            field: field.clone(),
        },
        Node::Ctor { name, args } => Node::Ctor {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| rewrite_refs(arg, ref_indices, depth))
                .collect(),
        },
        Node::Ann { expr, type_ } => Node::Ann {
            expr: Box::new(rewrite_refs(expr, ref_indices, depth)),
            type_: type_.clone(),
        },
        Node::Def { sig, body } => Node::Def {
            sig: sig.clone(),
            body: Box::new(rewrite_refs(body, ref_indices, depth)),
        },
        _ => node.clone(),
    }
}

fn detect_cycles(
    defs: &BTreeMap<String, &Node>,
    path: &[usize],
    aliases: &UnitAliases,
    diags: &mut Vec<Diagnostic>,
) {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Visiting,
        Done,
    }

    fn visit(
        hash: &str,
        defs: &BTreeMap<String, &Node>,
        marks: &mut BTreeMap<String, Mark>,
        stack: &mut Vec<String>,
        path: &[usize],
        aliases: &UnitAliases,
        diags: &mut Vec<Diagnostic>,
    ) {
        match marks.get(hash).copied() {
            Some(Mark::Done) => return,
            Some(Mark::Visiting) => {
                if let Some(start) = stack.iter().position(|h| h == hash) {
                    let mut cycle = stack[start..].to_vec();
                    cycle.push(hash.to_string());
                    let displayed = cycle
                        .iter()
                        .map(|hash| aliases.display_hash(hash))
                        .collect::<Vec<_>>();
                    diags.push(Diagnostic::cyclic_dependency(path, &displayed));
                }
                return;
            }
            None => {}
        }

        let Some(def) = defs.get(hash) else {
            return;
        };
        marks.insert(hash.to_string(), Mark::Visiting);
        stack.push(hash.to_string());
        if let Some(body) = definition_body(def) {
            let mut refs = Vec::new();
            collect_refs(body, &mut refs);
            for dep in refs {
                if defs.contains_key(&dep) {
                    visit(&dep, defs, marks, stack, path, aliases, diags);
                }
            }
        }
        stack.pop();
        marks.insert(hash.to_string(), Mark::Done);
    }

    let mut marks = BTreeMap::new();
    let mut stack = Vec::new();
    for hash in defs.keys() {
        visit(hash, defs, &mut marks, &mut stack, path, aliases, diags);
    }
}

fn collect_refs(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Ref { hash } => out.push(hash.clone()),
        _ => for_each_expr_child(node, |child| collect_refs(child, out)),
    }
}

fn for_each_expr_child(node: &Node, mut f: impl FnMut(&Node)) {
    match node {
        Node::Lam { body } => f(body),
        Node::App { fn_, arg } => {
            f(fn_);
            f(arg);
        }
        Node::Let { rhs, body } => {
            f(rhs);
            f(body);
        }
        Node::Rec { bindings, body } => {
            for binding in bindings {
                f(binding);
            }
            f(body);
        }
        Node::Module { bindings } => {
            for binding in bindings {
                f(binding);
            }
        }
        Node::If { cond, then, else_ } => {
            f(cond);
            f(then);
            f(else_);
        }
        Node::Match { scrutinee, arms } => {
            f(scrutinee);
            for arm in arms {
                f(arm);
            }
        }
        Node::Arm { pattern: _, body } => f(body),
        Node::Record { fields } => {
            for (_, value) in fields {
                f(value);
            }
        }
        Node::Proj { record, .. } => f(record),
        Node::Ctor { args, .. } => {
            for arg in args {
                f(arg);
            }
        }
        Node::Ann { expr, .. } => f(expr),
        Node::Def { body, .. } => f(body),
        Node::Unit { defs, .. } | Node::Defs { defs } => {
            for def in defs {
                f(def);
            }
        }
        _ => {}
    }
}

fn count_pat_vars(node: &Node) -> u64 {
    match node {
        Node::PatVar => 1,
        Node::PatCtor { sub_patterns, .. } => sub_patterns.iter().map(count_pat_vars).sum(),
        _ => 0,
    }
}

fn child_path(path: &[usize], suffix: &[usize]) -> Vec<usize> {
    let mut out = path.to_vec();
    out.extend_from_slice(suffix);
    out
}
