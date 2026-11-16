use derive_generic_visitor::*;
use std::collections::HashSet;

use crate::transform::TransformCtx;
use crate::ullbc_ast::*;

use crate::transform::ctx::UllbcPass;

pub struct Transform;
impl UllbcPass for Transform {
    fn transform_function(&self, ctx: &mut TransformCtx, decl: &mut FunDecl) {
        if !ctx.in_spec_body
            && let Some(pending) = ctx.pending_specs.get_mut(&decl.def_id)
        {
            for rvalue in pending.iter_captures_mut() {
                if let Rvalue::Use(Operand::Move(from) | Operand::Copy(from), _) = rvalue
                    && from.is_local()
                    && from.ty().is_unit()
                {
                    *rvalue = Rvalue::unit_value();
                }
            }
        }
        let mut spec_closures = std::mem::take(&mut ctx.translated.spec_closures);
        let prev_in_spec = ctx.in_spec_body;
        let mut is_host_body = true;
        decl.for_each_body(&mut spec_closures, |body, _| {
            ctx.in_spec_body = prev_in_spec || !std::mem::replace(&mut is_host_body, false);
            if let Some(body) = body.as_unstructured_mut() {
                self.transform_body(ctx, body);
            }
        });
        ctx.in_spec_body = prev_in_spec;
        ctx.translated.spec_closures = spec_closures;
    }

    fn transform_body(&self, ctx: &mut TransformCtx, body: &mut ExprBody) {
        let facts = crate::transform::borrowck::take(ctx, body);
        // Replace any copy/move of a unit local to a plain const assignment. Note: we don't touch
        // other `Rvalue`s as they might have side-effects (e.g. reading through a pointer).
        body.visit_statements(|st| {
            if let StatementKind::Assign(_, rvalue) = &mut st.kind
                && let Rvalue::Use(Operand::Move(from) | Operand::Copy(from), _) = rvalue
                && from.is_local()
                && from.ty().is_unit()
            {
                *rvalue = Rvalue::unit_value()
            }
        });

        // Find the unused locals of unit type.
        #[derive(Visitor)]
        struct UnitLocalsVisitor {
            unused_unit_locals: HashSet<LocalId>,
        }
        impl VisitBody for UnitLocalsVisitor {
            fn enter_place(&mut self, x: &Place) {
                if let Some(var_id) = x.as_local() {
                    self.unused_unit_locals.remove(&var_id);
                }
            }
            fn visit_ullbc_statement(
                &mut self,
                x: &ullbc_ast::Statement,
            ) -> ControlFlow<Self::Break> {
                match &x.kind {
                    StatementKind::Assign(place, rvalue) => {
                        if place.is_local() && place.ty().is_unit() {
                            // Don't count the assignment as a use.
                        } else {
                            self.visit(place)?;
                        }
                        self.visit(rvalue)?;
                    }
                    _ => self.visit_inner(x)?,
                }
                Continue(())
            }
        }
        let unused_unit_locals = (UnitLocalsVisitor {
            unused_unit_locals: body
                .locals
                .non_argument_locals()
                .filter(|(_, var)| var.ty.is_unit())
                .map(|(id, _)| id)
                .collect(),
        })
        .visit_by_val_infallible(&*body)
        .unused_unit_locals;

        // Remove side-effect-free assignments into unused places.
        body.visit_statements(|st| {
            if let StatementKind::Assign(place, rvalue) = &st.kind
                && let Some(var_id) = place.as_local()
                && unused_unit_locals.contains(&var_id)
                && rvalue.is_aggregate()
            {
                st.kind = StatementKind::Nop;
            }
        });
        crate::transform::borrowck::put(ctx, body, facts);
    }
}
