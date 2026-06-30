//! Remove the locals (which are not used for the input arguments) which are
//! never used in the function bodies.  This is useful to remove the locals with
//! type `Never`. We actually check that there are no such local variables
//! remaining afterwards.
use derive_generic_visitor::Visitor;
use std::mem;
use std::ops::ControlFlow::Continue;

use crate::ast::*;
use crate::ids::IndexVec;
use crate::transform::TransformCtx;
use crate::transform::ctx::{TransformPass, UllbcPass};

#[derive(Visitor)]
struct LocalsUsageVisitor<'a> {
    used_locals: IndexVec<LocalId, bool>,
    spec_closures: &'a IndexMap<SpecClosureId, SpecClosure>,
}

impl<'a> VisitBody for LocalsUsageVisitor<'a> {
    fn enter_local_id(&mut self, lid: &LocalId) {
        self.used_locals[*lid] = true;
    }
    fn visit_llbc_statement(&mut self, st: &llbc_ast::Statement) -> ControlFlow<Self::Break> {
        match &st.kind {
            llbc_ast::StatementKind::StorageDead(_) | llbc_ast::StatementKind::StorageLive(_) => {
                // These statements don't count as a variable use.
                Continue(())
            }
            _ => self.visit_inner(st),
        }
    }
    fn visit_ullbc_statement(&mut self, st: &ullbc_ast::Statement) -> ControlFlow<Self::Break> {
        match &st.kind {
            ullbc_ast::StatementKind::StorageDead(_) | ullbc_ast::StatementKind::StorageLive(_) => {
                // These statements don't count as a variable use.
                Continue(())
            }
            _ => self.visit_inner(st),
        }
    }
    fn visit_spec_closure(&mut self, spec_closure: &SpecClosure) -> ControlFlow<Self::Break> {
        self.visit(&spec_closure.captures)
    }
    fn visit_spec_closure_id(
        &mut self,
        spec_closure_id: &SpecClosureId,
    ) -> ControlFlow<Self::Break> {
        self.visit(&self.spec_closures[*spec_closure_id])
    }
}

#[derive(Visitor)]
struct LocalsRenumberVisitor {
    ids_map: IndexVec<LocalId, Option<LocalId>>,
}

impl VisitBodyMut for LocalsRenumberVisitor {
    fn enter_local_id(&mut self, lid: &mut LocalId) {
        *lid = self.ids_map[*lid].unwrap();
    }
    fn enter_llbc_statement(&mut self, st: &mut llbc_ast::Statement) {
        match st.kind {
            llbc_ast::StatementKind::StorageDead(lid)
            | llbc_ast::StatementKind::StorageLive(lid)
                if self.ids_map[lid].is_none() =>
            {
                st.kind = llbc_ast::StatementKind::Nop;
            }
            _ => {}
        }
    }
    fn enter_ullbc_statement(&mut self, st: &mut ullbc_ast::Statement) {
        match st.kind {
            ullbc_ast::StatementKind::StorageDead(lid)
            | ullbc_ast::StatementKind::StorageLive(lid)
                if self.ids_map[lid].is_none() =>
            {
                st.kind = ullbc_ast::StatementKind::Nop;
            }
            _ => {}
        }
    }
    fn visit_spec_closure(&mut self, spec_closure: &mut SpecClosure) -> ControlFlow<Self::Break> {
        self.visit(&mut spec_closure.captures)
    }
    fn visit_spec_closure_id(
        &mut self,
        _spec_closure_id: &mut SpecClosureId,
    ) -> ControlFlow<Self::Break> {
        // self.visit(&mut self.spec_closures[*spec_closure_id])
        ControlFlow::Continue(())
    }
}

fn remove_unused_locals<Body: BodyVisitable>(
    body: &mut GExprBody<Body>,
    spec_closures: &mut IndexMap<SpecClosureId, SpecClosure>,
) {
    // Compute the set of used locals.
    // We always register the return variable and the input arguments.
    let mut visitor = LocalsUsageVisitor {
        used_locals: body
            .locals
            .locals
            .map_ref(|local| body.locals.is_return_or_arg(local.index)),
        spec_closures,
    };
    let _ = body.body.drive_body(&mut visitor);
    let used_locals = visitor.used_locals;
    trace!("used_locals: {:?}", used_locals);

    // Keep only the variables that are used (storage statements don't count) and update their
    // indices to be contiguous.
    let mut ids_map: IndexVec<LocalId, Option<LocalId>> = body.locals.locals.map_ref(|_| None);
    for local in mem::take(&mut body.locals.locals) {
        if used_locals[local.index] {
            let old_id = local.index;
            let new_id = body
                .locals
                .locals
                .push_with(|index| Local { index, ..local });
            ids_map[old_id] = Some(new_id);
        }
    }
    trace!("ids_maps: {:?}", ids_map);

    // Update all `LocalId`s.
    let mut visitor = LocalsRenumberVisitor { ids_map };
    let _ = body.body.drive_body_mut(&mut visitor);
    body.body
        .dyn_visit_in_body(|spec_closure_id: &SpecClosureId| {
            spec_closures[*spec_closure_id].drive_body_mut(&mut visitor);
        });
}

pub struct Transform;
impl UllbcPass for Transform {
    fn transform_function(&self, ctx: &mut TransformCtx, decl: &mut FunDecl) {
        let spec_closures: &mut IndexMap<SpecClosureId, SpecClosure> =
            &mut ctx.translated.spec_closures;
        decl.for_each_body(spec_closures, |body, spec_closures| match body {
            Body::Unstructured(body) => remove_unused_locals(body, spec_closures),
            Body::Structured(body) => remove_unused_locals(body, spec_closures),
            _ => {}
        });
    }
}
impl TransformPass for Transform {
    fn transform_ctx(&self, ctx: &mut TransformCtx) {
        ctx.for_each_fun_decl(|ctx, fun| self.transform_function(ctx, fun));
    }
}
