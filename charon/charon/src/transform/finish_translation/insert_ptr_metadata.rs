/// We leave dummy values for pointer metadata during translation; instead we add it in this pass.
///
/// The reason to keep this as a pass is that `compute_place_metadata` indirectly queries the whole
/// crate to figure out the metadata, which we can't do during translation as some items may not be
/// translated yet. We could fetch some of that info via hax but this would then duplicate the
/// metadata computation code because it involves non-hax things like emitting new statements.
use crate::transform::{
    TransformCtx,
    ctx::{BodyTransformCtx, UllbcPass, UllbcStatementTransformCtx},
};
use crate::ullbc_ast::*;

pub struct Transform;

impl UllbcPass for Transform {
    fn transform_function(&self, ctx: &mut TransformCtx, decl: &mut FunDecl) {
        let mut facts = decl
            .body
            .as_unstructured()
            .and_then(|b| crate::transform::borrowck::take(ctx, b));
        let shape = decl.body.as_unstructured().map(|b| {
            (
                b.locals.locals.len(),
                b.body
                    .iter()
                    .map(|b| b.statements.len())
                    .collect::<Vec<_>>(),
            )
        });
        decl.transform_ullbc_statements(ctx, |ctx, st: &mut Statement| {
            st.dyn_visit_in_body_mut(|x: &mut Rvalue| {
                if let Rvalue::Ref {
                    place,
                    ptr_metadata,
                    ..
                }
                | Rvalue::RawPtr {
                    place,
                    ptr_metadata,
                    ..
                } = x
                {
                    *ptr_metadata = ctx.compute_place_metadata(place);
                }
            });
        });
        if !ctx.in_spec_body
            && let Some(body) = decl.body.as_unstructured_mut()
            && let Some(mut pending) = ctx.pending_specs.remove(&decl.def_id)
        {
            let mut stmt_ctx = UllbcStatementTransformCtx {
                ctx,
                params: &decl.generics,
                locals: &mut body.locals,
                span: decl.item_meta.span,
                statements: Vec::new(),
            };
            for rvalue in pending.iter_captures_mut() {
                if let Rvalue::Ref {
                    place,
                    ptr_metadata,
                    ..
                }
                | Rvalue::RawPtr {
                    place,
                    ptr_metadata,
                    ..
                } = rvalue
                {
                    *ptr_metadata = stmt_ctx.compute_place_metadata(place);
                }
            }
            debug_assert!(
                stmt_ctx.statements.is_empty(),
                "closure capture metadata must not emit host statements"
            );
            ctx.pending_specs.insert(decl.def_id, pending);
        }
        if let Some(body) = decl.body.as_unstructured() {
            if shape
                != Some((
                    body.locals.locals.len(),
                    body.body.iter().map(|b| b.statements.len()).collect(),
                ))
                && let Some(info) = facts.as_mut()
            {
                info.unsupported =
                    Some("borrowck: pointer metadata expansion is not yet supported".into());
            }
            crate::transform::borrowck::put(ctx, body, facts);
        }
    }
}
