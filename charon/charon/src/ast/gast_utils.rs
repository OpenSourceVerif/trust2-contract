//! Implementations for [crate::gast]

use crate::ast::*;

use std::mem;

impl FnPtrKind {
    pub fn mk_builtin(aid: BuiltinFunId) -> Self {
        Self::Fun(FunId::Builtin(aid))
    }
}

impl Body {
    /// Whether there is an actual body with statements etc, as opposed to the body being missing
    /// for some reason.
    pub fn has_contents(&self) -> bool {
        match self {
            Body::Unstructured(..) | Body::Structured(..) => true,
            Body::TraitMethodWithoutDefault
            | Body::Extern(..)
            | Body::Intrinsic { .. }
            | Body::Opaque
            | Body::Missing
            | Body::Error(..)
            | Body::TargetDispatch(..) => false,
        }
    }

    pub fn locals(&self) -> &Locals {
        match self {
            Body::Structured(body) => &body.locals,
            Body::Unstructured(body) => &body.locals,
            _ => panic!("called `locals` on a missing body"),
        }
    }

    pub fn for_each_body(
        &mut self,
        spec_closures: &mut IndexMap<SpecClosureId, SpecClosure>,
        mut f: impl FnMut(&mut Body, &mut IndexMap<SpecClosureId, SpecClosure>),
    ) {
        self.for_each_body_inner(spec_closures, &mut f);
    }

    fn for_each_body_inner(
        &mut self,
        spec_closures: &mut IndexMap<SpecClosureId, SpecClosure>,
        f: &mut impl FnMut(&mut Body, &mut IndexMap<SpecClosureId, SpecClosure>),
    ) {
        f(self, spec_closures);
        self.dyn_visit_in_body(|&spec_closure_id: &SpecClosureId| {
            let mut body = mem::replace(&mut spec_closures[spec_closure_id].body, Body::Opaque);
            body.for_each_body_inner(spec_closures, f);
            spec_closures[spec_closure_id].body = body;
        });
    }
}

impl Locals {
    pub fn new(arg_count: usize) -> Self {
        Self {
            arg_count,
            locals: Default::default(),
        }
    }

    /// Creates a new variable and returns a place pointing to it.
    /// Warning: don't forget to `StorageLive` it before using it.
    pub fn new_var(&mut self, name: Option<String>, ty: Ty) -> Place {
        let local_id = self.locals.push_with(|index| Local {
            index,
            name,
            span: Span::dummy(),
            ty: ty.clone(),
        });
        Place::new(local_id, ty)
    }

    /// Gets a place pointing to the corresponding variable.
    pub fn place_for_var(&self, local_id: LocalId) -> Place {
        let ty = self.locals[local_id].ty.clone();
        Place::new(local_id, ty)
    }

    /// Returns whether this local is the special return local or one of the input argument locals.
    pub fn is_return_or_arg(&self, lid: LocalId) -> bool {
        lid.index() <= self.arg_count
    }

    /// The place where we write the return value.
    pub fn return_place(&self) -> Place {
        self.place_for_var(LocalId::new(0))
    }

    /// Locals that aren't arguments or return values.
    pub fn non_argument_locals(&self) -> impl Iterator<Item = (LocalId, &Local)> {
        self.locals.iter_enumerated().skip(1 + self.arg_count)
    }
}

impl std::ops::Index<LocalId> for Locals {
    type Output = Local;
    fn index(&self, local_id: LocalId) -> &Self::Output {
        &self.locals[local_id]
    }
}
impl std::ops::IndexMut<LocalId> for Locals {
    fn index_mut(&mut self, local_id: LocalId) -> &mut Self::Output {
        &mut self.locals[local_id]
    }
}

impl SpecClosure {
    pub fn non_captured_argument_ids(&self) -> impl Iterator<Item = usize> {
        (1..=self.body.locals().arg_count)
            .filter(|&local_id| self.captures.get(local_id.into()).is_none())
    }
}

impl FunSpecs {
    pub fn new() -> Self {
        Self {
            preconditions: Vec::new(),
            postconditions: Vec::new(),
        }
    }

    pub fn for_each_body(
        &mut self,
        spec_closures: &mut IndexMap<SpecClosureId, SpecClosure>,
        mut f: impl FnMut(&mut Body, &mut IndexMap<SpecClosureId, SpecClosure>),
    ) {
        self.preconditions
            .iter_mut()
            .chain(&mut self.postconditions)
            .for_each(|spec_closure| spec_closure.body.for_each_body(spec_closures, &mut f));
    }
}

impl Default for FunSpecs {
    fn default() -> Self {
        Self::new()
    }
}

impl FunDecl {
    /// Replace the generic parameters of this function with the ones given by the binder.
    pub fn substitute_params(self, subst: Binder<GenericArgs>) -> Self {
        let FunDecl {
            def_id,
            item_meta,
            generics: _,
            signature,
            src,
            is_global_initializer,
            body,
            specs,
        } = self;
        let signature = signature.substitute(&subst.skip_binder);
        let src = src.substitute(&subst.skip_binder);
        let body = body.substitute(&subst.skip_binder);
        let specs = specs.substitute(&subst.skip_binder);
        FunDecl {
            def_id,
            item_meta,
            generics: subst.params,
            signature,
            src,
            is_global_initializer,
            body,
            specs,
        }
    }

    pub fn for_each_body(
        &mut self,
        spec_closures: &mut IndexMap<SpecClosureId, SpecClosure>,
        mut f: impl FnMut(&mut Body, &mut IndexMap<SpecClosureId, SpecClosure>),
    ) {
        self.body.for_each_body(spec_closures, &mut f);
        self.specs.for_each_body(spec_closures, f);
    }
}
impl TraitDecl {
    pub fn methods(&self) -> impl Iterator<Item = &Binder<TraitMethod>> {
        self.methods.iter()
    }
}
impl TraitImpl {
    pub fn methods(&self) -> impl Iterator<Item = &Binder<FunDeclRef>> {
        self.methods.iter()
    }
}

impl Binder<TraitAssocTy> {
    pub fn name(&self) -> &TraitItemName {
        &self.skip_binder.name
    }
}
impl Binder<TraitMethod> {
    pub fn name(&self) -> TraitItemName {
        self.skip_binder.name
    }
}
