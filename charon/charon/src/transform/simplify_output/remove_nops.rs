//! Remove the useless no-ops.
use crate::ast::*;
use crate::transform::TransformCtx;

use crate::transform::ctx::TransformPass;

pub struct Transform;
impl TransformPass for Transform {
    fn transform_ctx(&self, ctx: &mut TransformCtx) {
        ctx.for_each_body(|ctx, body| {
            match body {
                Body::Unstructured(body) => {
                    let mut facts = crate::transform::borrowck::take(ctx, body);
                    if let Some(info) = facts.as_mut() {
                        crate::transform::borrowck::remap(info, |mut p| {
                            let block = &body.body[p.block];
                            if block
                                .statements
                                .get(p.statement_index)
                                .is_some_and(|s| s.kind.is_nop())
                            {
                                return None;
                            }
                            p.statement_index -= block.statements[..p.statement_index]
                                .iter()
                                .filter(|s| s.kind.is_nop())
                                .count();
                            Some(p)
                        });
                    }
                    for blk in &mut body.body {
                        if blk.statements.iter().any(|st| st.kind.is_nop()) {
                            blk.statements.retain(|st| !st.kind.is_nop())
                        }
                    }
                    crate::transform::borrowck::put(ctx, body, facts);
                }
                Body::Structured(body) => {
                    body.body.visit_blocks_bwd(|blk: &mut llbc_ast::Block| {
                        // Remove all the `Nop`s from this sequence.
                        if blk.statements.iter().any(|st| st.kind.is_nop()) {
                            blk.statements.retain(|st| !st.kind.is_nop())
                        }
                    });
                }
                _ => {}
            }
        });
    }
}
