//! Remove the useless no-ops.
use crate::ast::*;
use crate::transform::TransformCtx;

use crate::transform::ctx::TransformPass;

trait IsNop {
    fn is_nop(&self) -> bool;
}

impl IsNop for ullbc_ast::Statement {
    fn is_nop(&self) -> bool {
        self.kind.is_nop()
    }
}

impl IsNop for llbc_ast::Statement {
    fn is_nop(&self) -> bool {
        self.kind.is_nop()
    }
}

fn remove_nop_statements<S: IsNop>(statements: &mut Vec<S>) {
    // Remove all the `Nop`s from this sequence.
    if statements.iter().any(|st| st.is_nop()) {
        statements.retain(|st| !st.is_nop());
    }
}

pub struct Transform;
impl TransformPass for Transform {
    fn transform_ctx(&self, ctx: &mut TransformCtx) {
        ctx.for_each_fun_decl(|_ctx, fun| {
            match &mut fun.body {
                Body::Unstructured(body) => {
                    for blk in &mut body.body {
                        remove_nop_statements(&mut blk.statements);
                    }
                    for blk in body
                        .specs
                        .preconditions
                        .iter_mut()
                        .chain(&mut body.specs.postconditions)
                    {
                        remove_nop_statements(&mut blk.statements);
                    }
                }
                Body::Structured(body) => {
                    body.body.visit_blocks_bwd(|blk: &mut llbc_ast::Block| {
                        remove_nop_statements(&mut blk.statements);
                    });
                    for blk in body
                        .specs
                        .preconditions
                        .iter_mut()
                        .chain(&mut body.specs.postconditions)
                    {
                        remove_nop_statements(&mut blk.statements);
                    }
                }
                _ => {}
            }
        });
    }
}
