//! Replace variables bound at the top-level with `Free` vars. This is for convenience for
//! consumers of the charon ast.
use std::mem;

use derive_generic_visitor::*;
use index_vec::Idx;

use crate::ast::*;

use crate::transform::{TransformCtx, ctx::TransformPass};

/// Replace variables bound at the top-level with `Free` vars.
#[derive(Visitor)]
pub(crate) struct UnbindVarVisitor<'a> {
    // Tracks the depth of binders we're inside of.
    binder_depth: DeBruijnId,
    spec_bodies: &'a mut IndexMap<SpecBodyId, Body>,
    spec_closures: &'a mut IndexMap<SpecClosureId, SpecClosure>,
}

impl VisitorWithBinderDepth for UnbindVarVisitor<'_> {
    fn binder_depth_mut(&mut self) -> &mut DeBruijnId {
        &mut self.binder_depth
    }
}
impl VisitAstMut for UnbindVarVisitor<'_> {
    fn visit<T: AstVisitable>(&mut self, x: &mut T) -> ControlFlow<Self::Break> {
        VisitWithBinderDepth::new(self).visit(x)
    }

    fn exit_de_bruijn_var<T: AstVisitable + Idx>(&mut self, var: &mut DeBruijnVar<T>) {
        match var {
            DeBruijnVar::Bound(dbid, varid) if *dbid == self.binder_depth => {
                *var = DeBruijnVar::Free(*varid)
            }
            DeBruijnVar::Bound(..) => {}
            DeBruijnVar::Free(_) => unreachable!("Found unexpected free variable"),
        }
    }

    fn visit_spec_body_id(&mut self, spec_body_id: &mut SpecBodyId) -> ControlFlow<Self::Break> {
        let mut spec_body = mem::replace(&mut self.spec_bodies[*spec_body_id], Body::Opaque);
        let result = self.visit(&mut spec_body);
        self.spec_bodies[*spec_body_id] = spec_body;
        result
    }
    fn visit_spec_closure_id(
        &mut self,
        spec_closure_id: &mut SpecClosureId,
    ) -> ControlFlow<Self::Break> {
        let mut spec_closure = mem::replace(
            &mut self.spec_closures[*spec_closure_id],
            SpecClosure {
                captures: Default::default(),
                body: Body::Opaque,
            },
        );
        let result = self.visit(&mut spec_closure);
        self.spec_closures[*spec_closure_id] = spec_closure;
        result
    }
}

pub struct Check;
impl TransformPass for Check {
    fn should_run(&self, options: &crate::options::TranslateOptions) -> bool {
        options.unbind_item_vars
    }

    fn transform_ctx(&self, ctx: &mut TransformCtx) {
        let mut spec_bodies = mem::take(&mut ctx.translated.spec_bodies);
        let mut spec_closures = mem::take(&mut ctx.translated.spec_closures);
        let mut visitor = UnbindVarVisitor {
            binder_depth: Default::default(),
            spec_closures: &mut spec_closures,
            spec_bodies: &mut spec_bodies,
        };
        for mut item in ctx.translated.all_items_mut() {
            let _ = item.drive_mut(&mut visitor);
        }
        ctx.translated.spec_closures = spec_closures;
        ctx.translated.spec_bodies = spec_bodies;
    }
}
