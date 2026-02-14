use crate::{
    ast::{BuiltinFunId, Call, FnOperand, FnPtrKind, FunId, SpecCall},
    transform::{TransformCtx, ctx::UllbcPass},
    ullbc_ast::{BlockData, FunSpecBlock, Terminator, TerminatorKind},
};

use std::mem;

pub struct Transform;
impl UllbcPass for Transform {
    fn transform_function(&self, _ctx: &mut TransformCtx, decl: &mut crate::ast::FunDecl) {
        let Some(body) = decl.body.as_unstructured_mut() else {
            return;
        };

        for block_data in &mut body.body {
            let Terminator {
                span,
                kind:
                    TerminatorKind::Call {
                        call:
                            Call {
                                ref func,
                                ref mut args,
                                ..
                            },
                        target,
                        ..
                    },
                ..
            } = block_data.terminator
            else {
                continue;
            };
            let FnOperand::Regular(fn_ptr) = func else {
                continue;
            };
            let FnPtrKind::Fun(FunId::Builtin(fun_id)) = fn_ptr.kind else {
                continue;
            };

            if matches!(fun_id, BuiltinFunId::SpecEntry) {
                block_data.terminator.kind = TerminatorKind::Goto { target };
            } else if matches!(
                fun_id,
                BuiltinFunId::SpecPrecondition | BuiltinFunId::SpecPostcondition
            ) {
                let args = mem::take(args);
                let new_block_data = BlockData::new_goto(span, target);
                let old_block_data = mem::replace(block_data, new_block_data);
                let spec_block = FunSpecBlock {
                    statements: old_block_data.statements,
                    call: SpecCall { span, args },
                };
                if matches!(fun_id, BuiltinFunId::SpecPrecondition) {
                    body.specs.preconditions.push(spec_block);
                } else {
                    body.specs.postconditions.push(spec_block);
                }
            }
        }
    }
}
