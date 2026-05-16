//! Type and effect representation for Phase 2 type and effect inference.

use std::collections::{BTreeMap, BTreeSet, HashMap};

// ── Effect atoms ─────────────────────────────────────────────────────────────

/// One of the four fixed atomic effects (ADR 0035).  Sorted alphabetically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EffAtom {
    Alloc,
    Div,
    IO,
    Mut,
}

impl std::fmt::Display for EffAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffAtom::Alloc => write!(f, "Alloc"),
            EffAtom::Div => write!(f, "Div"),
            EffAtom::IO => write!(f, "IO"),
            EffAtom::Mut => write!(f, "Mut"),
        }
    }
}

/// A concrete, sorted set of atomic effects.  Empty = pure (bottom).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffSet {
    pub atoms: BTreeSet<EffAtom>,
}

impl EffSet {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn of(atoms: impl IntoIterator<Item = EffAtom>) -> Self {
        Self {
            atoms: atoms.into_iter().collect(),
        }
    }

    pub fn join(&self, other: &Self) -> Self {
        Self {
            atoms: self.atoms.union(&other.atoms).cloned().collect(),
        }
    }

    pub fn is_subset_of(&self, other: &Self) -> bool {
        self.atoms.is_subset(&other.atoms)
    }

    pub fn is_pure(&self) -> bool {
        self.atoms.is_empty()
    }
}

impl std::fmt::Display for EffSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_pure() {
            write!(f, "{{}}")
        } else {
            let parts: Vec<_> = self.atoms.iter().map(|a| a.to_string()).collect();
            write!(f, "{{{}}}", parts.join(", "))
        }
    }
}

// ── Effect annotation on a function type ────────────────────────────────────

/// The effect annotation on a function call, or the eval-effect of an expression.
/// Can be a concrete set, or an unresolved metavariable (from effect polymorphism).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FnEff {
    /// Concrete effect set — the common case.
    Concrete(EffSet),
    /// Effect metavariable, resolved during inference.
    /// The eff_map in `Subst` may chain metas: Meta(a) → Meta(b) → Concrete(s).
    Meta(u32),
}

impl FnEff {
    pub fn pure_() -> Self {
        Self::Concrete(EffSet::empty())
    }
    pub fn from_set(set: EffSet) -> Self {
        Self::Concrete(set)
    }
}

impl std::fmt::Display for FnEff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FnEff::Concrete(eff) => write!(f, " / {}", eff),
            FnEff::Meta(i) => write!(f, " / ?e{}", i),
        }
    }
}

/// Join two `FnEff` values, taking the union of their effects.
///
/// Rules:
/// - Concrete ∪ Concrete = Concrete (set union)
/// - pure ∪ Meta = Meta (pure adds nothing; meta may have atoms)
/// - non-pure Concrete ∪ Meta = Concrete (meta might be pure; take the concrete atoms)
/// - Meta ∪ Meta = first meta (metas will be linked via the substitution)
pub fn join_fn_eff(e1: &FnEff, e2: &FnEff, subst: &Subst) -> FnEff {
    let r1 = subst.apply_eff(e1);
    let r2 = subst.apply_eff(e2);
    match (&r1, &r2) {
        (FnEff::Concrete(s1), FnEff::Concrete(s2)) => FnEff::Concrete(s1.join(s2)),
        (FnEff::Concrete(s), FnEff::Meta(_)) if s.is_pure() => r2.clone(),
        (FnEff::Meta(_), FnEff::Concrete(s)) if s.is_pure() => r1.clone(),
        (FnEff::Concrete(s), FnEff::Meta(_)) => FnEff::Concrete(s.clone()),
        (FnEff::Meta(_), FnEff::Concrete(s)) => FnEff::Concrete(s.clone()),
        (FnEff::Meta(id), FnEff::Meta(_)) => FnEff::Meta(*id),
    }
}

// ── Value types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntSign {
    Signed,
    Unsigned,
}

impl IntSign {
    pub fn prefix(self) -> &'static str {
        match self {
            IntSign::Signed => "i",
            IntSign::Unsigned => "u",
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(self, IntSign::Signed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedIntTy {
    pub sign: IntSign,
    pub width: u16,
}

impl FixedIntTy {
    pub const fn new(sign: IntSign, width: u16) -> Self {
        Self { sign, width }
    }

    pub fn parse_name(name: &str) -> Option<Self> {
        let (sign, width_text) = match name.as_bytes().first().copied()? {
            b'i' => (IntSign::Signed, &name[1..]),
            b'u' => (IntSign::Unsigned, &name[1..]),
            _ => return None,
        };
        let width: u16 = width_text.parse().ok()?;
        matches!(width, 8 | 16 | 32 | 64).then_some(Self { sign, width })
    }

    pub fn name(self) -> String {
        format!("{}{}", self.sign.prefix(), self.width)
    }

    pub fn is_i64(self) -> bool {
        self.sign == IntSign::Signed && self.width == 64
    }
}

impl std::fmt::Display for FixedIntTy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.sign.prefix(), self.width)
    }
}

/// A Tacit-Lite type.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    /// Untyped integer literal before contextual defaulting.
    IntLit,
    Int,
    Bool,
    Str,
    /// Writable buffer handle (ADR 0038).
    Buf,
    /// Opaque i64 vector handle (ADR 0061).
    I64Vec,
    /// Fixed-width signed or unsigned integer (ADR 0084).
    FixedInt(FixedIntTy),
    /// Length-carrying typed mutable vector handle (ADR 0085).
    Vec(FixedIntTy),
    /// Function type `arg → ret / eff`.
    Fn(Box<Ty>, Box<Ty>, FnEff),
    Record(BTreeMap<String, Ty>),
    App(Box<Ty>, Box<Ty>),
    /// Unification metavariable.
    Meta(u32),
    /// Error-recovery type: prevents cascading errors.
    Unknown,
}

impl Ty {
    pub fn is_unknown(&self) -> bool {
        matches!(self, Ty::Unknown)
    }

    pub fn is_ground(&self, subst: &Subst) -> bool {
        match subst.apply(self) {
            Ty::IntLit
            | Ty::Int
            | Ty::Bool
            | Ty::Str
            | Ty::Buf
            | Ty::I64Vec
            | Ty::FixedInt(_)
            | Ty::Vec(_)
            | Ty::Unknown => true,
            Ty::Fn(a, b, _) => a.is_ground(subst) && b.is_ground(subst),
            Ty::Record(fields) => fields.values().all(|v| v.is_ground(subst)),
            Ty::App(f, a) => f.is_ground(subst) && a.is_ground(subst),
            Ty::Meta(_) => false,
        }
    }
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::IntLit => write!(f, "Int"),
            Ty::Int => write!(f, "Int"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Str => write!(f, "Str"),
            Ty::Buf => write!(f, "Buf"),
            Ty::I64Vec => write!(f, "I64Vec"),
            Ty::FixedInt(int_ty) => write!(f, "{}", int_ty),
            Ty::Vec(int_ty) => write!(f, "{}vec", int_ty),
            Ty::Fn(a, b, eff) => {
                let parens = matches!(a.as_ref(), Ty::Fn(_, _, _));
                if parens {
                    write!(f, "({}) -> {}{}", a, b, eff)
                } else {
                    write!(f, "{} -> {}{}", a, b, eff)
                }
            }
            Ty::Record(fields) => {
                write!(f, "{{")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Ty::App(fun, arg) => write!(f, "{} {}", fun, arg),
            Ty::Meta(n) => write!(f, "?{}", n),
            Ty::Unknown => write!(f, "<unknown>"),
        }
    }
}

// ── Substitution ──────────────────────────────────────────────────────────────

/// Combined type and effect substitution.
///
/// `eff_map` maps effect metavariable IDs to `FnEff`, supporting meta-to-meta
/// links for effect polymorphism (e.g. `?e0 → Meta(?e1) → Concrete({IO})`).
#[derive(Default)]
pub struct Subst {
    ty_map: HashMap<u32, Ty>,
    eff_map: HashMap<u32, FnEff>,
    next_ty: u32,
    next_eff: u32,
}

impl Subst {
    pub fn fresh(&mut self) -> Ty {
        let id = self.next_ty;
        self.next_ty += 1;
        Ty::Meta(id)
    }

    pub fn fresh_eff(&mut self) -> FnEff {
        let id = self.next_eff;
        self.next_eff += 1;
        FnEff::Meta(id)
    }

    /// Apply the type substitution, following metavariable chains.
    /// Also applies the effect substitution inside `Ty::Fn` nodes.
    pub fn apply(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Meta(id) => {
                if let Some(t) = self.ty_map.get(id) {
                    self.apply(t)
                } else {
                    ty.clone()
                }
            }
            Ty::Fn(a, b, eff) => Ty::Fn(
                Box::new(self.apply(a)),
                Box::new(self.apply(b)),
                self.apply_eff(eff),
            ),
            Ty::Record(fields) => Ty::Record(
                fields
                    .iter()
                    .map(|(k, v)| (k.clone(), self.apply(v)))
                    .collect(),
            ),
            Ty::App(fun, arg) => Ty::App(Box::new(self.apply(fun)), Box::new(self.apply(arg))),
            other => other.clone(),
        }
    }

    /// Apply the effect substitution, following meta chains.
    pub fn apply_eff(&self, eff: &FnEff) -> FnEff {
        match eff {
            FnEff::Meta(id) => {
                if let Some(target) = self.eff_map.get(id) {
                    self.apply_eff(target)
                } else {
                    eff.clone()
                }
            }
            FnEff::Concrete(_) => eff.clone(),
        }
    }

    /// Resolve a `FnEff` to a concrete `EffSet`, defaulting to empty if unresolved.
    pub fn resolve_eff(&self, eff: &FnEff) -> EffSet {
        match self.apply_eff(eff) {
            FnEff::Concrete(s) => s,
            FnEff::Meta(_) => EffSet::empty(),
        }
    }

    pub fn bind(&mut self, id: u32, ty: Ty) {
        self.ty_map.insert(id, ty);
    }

    /// Bind an effect metavariable to a `FnEff` (which may be another meta or a concrete set).
    pub fn bind_eff(&mut self, id: u32, target: FnEff) {
        self.eff_map.insert(id, target);
    }
}

// ── Occur check ───────────────────────────────────────────────────────────────

pub fn occurs(id: u32, ty: &Ty, subst: &Subst) -> bool {
    match subst.apply(ty) {
        Ty::Meta(other) => other == id,
        Ty::Fn(a, b, _) => occurs(id, &a, subst) || occurs(id, &b, subst),
        Ty::Record(fields) => fields.values().any(|v| occurs(id, v, subst)),
        Ty::App(f, a) => occurs(id, &f, subst) || occurs(id, &a, subst),
        _ => false,
    }
}

// ── Unification ───────────────────────────────────────────────────────────────

pub fn unify(t1: &Ty, t2: &Ty, subst: &mut Subst) -> bool {
    let t1 = subst.apply(t1);
    let t2 = subst.apply(t2);

    match (&t1, &t2) {
        (Ty::Unknown, _) | (_, Ty::Unknown) => true,
        (Ty::IntLit, Ty::IntLit)
        | (Ty::IntLit, Ty::Int)
        | (Ty::Int, Ty::IntLit)
        | (Ty::IntLit, Ty::FixedInt(_))
        | (Ty::FixedInt(_), Ty::IntLit)
        | (Ty::Int, Ty::Int)
        | (Ty::Bool, Ty::Bool)
        | (Ty::Str, Ty::Str)
        | (Ty::Buf, Ty::Buf)
        | (Ty::I64Vec, Ty::I64Vec) => true,
        (Ty::FixedInt(a), Ty::FixedInt(b)) => a == b,
        (Ty::Int, Ty::FixedInt(fixed)) | (Ty::FixedInt(fixed), Ty::Int) if fixed.is_i64() => true,
        (Ty::Vec(a), Ty::Vec(b)) => a == b,
        (Ty::Meta(id1), Ty::Meta(id2)) if id1 == id2 => true,
        (Ty::Meta(id), other) => {
            if occurs(*id, other, subst) {
                false
            } else {
                subst.bind(*id, other.clone());
                true
            }
        }
        (other, Ty::Meta(id)) => {
            if occurs(*id, other, subst) {
                false
            } else {
                subst.bind(*id, other.clone());
                true
            }
        }
        (Ty::Fn(a1, b1, e1), Ty::Fn(a2, b2, e2)) => {
            let (a1, b1, e1) = (a1.clone(), b1.clone(), e1.clone());
            let (a2, b2, e2) = (a2.clone(), b2.clone(), e2.clone());
            let ok = unify(&a1, &a2, subst) && unify(&b1, &b2, subst);
            unify_eff(&e1, &e2, subst);
            ok
        }
        (Ty::Record(f1), Ty::Record(f2)) => {
            if f1.len() != f2.len() {
                return false;
            }
            let pairs: Vec<_> = f1
                .iter()
                .zip(f2.iter())
                .map(|((k1, v1), (k2, v2))| (k1.clone(), v1.clone(), k2.clone(), v2.clone()))
                .collect();
            pairs
                .iter()
                .all(|(k1, v1, k2, v2)| k1 == k2 && unify(v1, v2, subst))
        }
        (Ty::App(f1, a1), Ty::App(f2, a2)) => {
            let (f1, a1, f2, a2) = (f1.clone(), a1.clone(), f2.clone(), a2.clone());
            unify(&f1, &f2, subst) && unify(&a1, &a2, subst)
        }
        _ => false,
    }
}

/// Unify two effect annotations.
///
/// - Meta + Concrete: bind the meta to the concrete set.
/// - Meta + Meta: link one meta to the other (chain).
/// - Concrete + Concrete: no binding needed.
pub fn unify_eff(e1: &FnEff, e2: &FnEff, subst: &mut Subst) {
    let r1 = subst.apply_eff(e1);
    let r2 = subst.apply_eff(e2);
    match (&r1, &r2) {
        (FnEff::Concrete(_), FnEff::Concrete(_)) => {}
        (FnEff::Meta(id1), FnEff::Meta(id2)) if id1 == id2 => {}
        (FnEff::Meta(id), other) => {
            subst.bind_eff(*id, other.clone());
        }
        (other, FnEff::Meta(id)) => {
            subst.bind_eff(*id, other.clone());
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_gives_distinct_ids() {
        let mut s = Subst::default();
        assert_ne!(s.fresh(), s.fresh());
    }

    #[test]
    fn fresh_eff_gives_distinct_ids() {
        let mut s = Subst::default();
        assert_ne!(s.fresh_eff(), s.fresh_eff());
    }

    #[test]
    fn unify_int_int() {
        let mut s = Subst::default();
        assert!(unify(&Ty::Int, &Ty::Int, &mut s));
    }

    #[test]
    fn unify_int_bool_fails() {
        let mut s = Subst::default();
        assert!(!unify(&Ty::Int, &Ty::Bool, &mut s));
    }

    #[test]
    fn unify_meta_int() {
        let mut s = Subst::default();
        let m = s.fresh();
        assert!(unify(&m, &Ty::Int, &mut s));
        assert_eq!(s.apply(&m), Ty::Int);
    }

    #[test]
    fn unify_fn_types() {
        let mut s = Subst::default();
        let m = s.fresh();
        let t1 = Ty::Fn(Box::new(Ty::Int), Box::new(m.clone()), FnEff::pure_());
        let t2 = Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Bool), FnEff::pure_());
        assert!(unify(&t1, &t2, &mut s));
        assert_eq!(s.apply(&m), Ty::Bool);
    }

    #[test]
    fn unify_fn_types_with_eff_meta() {
        let mut s = Subst::default();
        let em = s.fresh_eff();
        let io_eff = FnEff::Concrete(EffSet::of([EffAtom::IO]));
        let t1 = Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int), em.clone());
        let t2 = Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int), io_eff.clone());
        assert!(unify(&t1, &t2, &mut s));
        assert_eq!(s.apply_eff(&em), io_eff);
    }

    #[test]
    fn eff_meta_to_meta_link() {
        let mut s = Subst::default();
        let e0 = s.fresh_eff();
        let e1 = s.fresh_eff();
        // Link e0 → e1
        unify_eff(&e0, &e1, &mut s);
        // Now bind e1 to IO
        let io = FnEff::Concrete(EffSet::of([EffAtom::IO]));
        unify_eff(&e1, &io, &mut s);
        // e0 should follow chain to IO
        assert_eq!(s.apply_eff(&e0), io);
    }

    #[test]
    fn occur_check_prevents_infinite_type() {
        let mut s = Subst::default();
        let m = s.fresh();
        let recursive = Ty::Fn(Box::new(m.clone()), Box::new(m.clone()), FnEff::pure_());
        assert!(!unify(&m, &recursive, &mut s));
    }

    #[test]
    fn eff_set_join() {
        let io = EffSet::of([EffAtom::IO]);
        let div = EffSet::of([EffAtom::Div]);
        let joined = io.join(&div);
        assert!(joined.atoms.contains(&EffAtom::IO));
        assert!(joined.atoms.contains(&EffAtom::Div));
    }

    #[test]
    fn eff_set_subset() {
        let io = EffSet::of([EffAtom::IO]);
        let io_div = EffSet::of([EffAtom::IO, EffAtom::Div]);
        assert!(io.is_subset_of(&io_div));
        assert!(!io_div.is_subset_of(&io));
    }
}
