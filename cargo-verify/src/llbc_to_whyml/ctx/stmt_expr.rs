//! 把语句翻译成可执行代码（`Expr`）。
//!
//! 与 `stmt_term` 不同，这里翻译的是函数体本身：局部变量用 `ref` 模拟，
//! 循环用 `while true` 加异常（`break`/`continue`）表示，`return` 用异常实现。

use super::Ctx;

use itertools::Itertools;

use crate::llbc_to_whyml::{
    error::{Error, Result},
    ident::{EQUAL, RETURN_LABEL, break_exn_ident, continue_exn_ident, local_temp_ident},
    prelude::{ABSURD_EXPR, TRUE_EXPR, UNIT_EXPR, UNIT_TYPE},
};

use charon_lib::{
    ast::{
        AbortKind, Assert, Call, ContractAssertKind, CopyNonOverlapping, FnOperand, LocalId,
        Operand, Place, Rvalue, SpecClosureId, VariantId,
    },
    llbc_ast::{Block, Statement, StatementKind, Switch},
};

use why3_ptree::{
    expr::{AssertionKind, RsKind},
    ity::Mask,
    loc::Position,
    ptree::{Expr, ExprDesc, Ident, Qualid},
    ptree_helpers,
};

impl<'a> Ctx<'a> {
    /// 把一个基本块翻译成表达式，其值是该块的返回值。
    /// 语句是逆序处理的：后一条语句产生的表达式作为前一条语句的「后续表达式」。
    pub(super) fn translate_block_to_expr(&mut self, block: &Block) -> Result<Expr> {
        let mut expr = UNIT_EXPR.clone();
        for stmt in block.statements.iter().rev() {
            expr = self.translate_statement_to_expr(stmt, expr)?;
        }
        Ok(expr)
    }

    /// 把一条语句翻译成表达式。
    ///
    /// `trailing_expr` 是这条语句之后的所有语句翻译出的表达式：顺序执行
    /// 被翻译成 `Sequence`，赋值被翻译成 `Assign`，控制流结构（`if`/`match`/`loop`）
    /// 直接对应 WhyML 的相应构造。
    fn translate_statement_to_expr(
        &mut self,
        stmt: &Statement,
        trailing_expr: Expr,
    ) -> Result<Expr> {
        fn sequence_expr(trailing_expr: Expr, expr: Expr) -> Expr {
            ptree_helpers::expr(
                Position::default(),
                ExprDesc::Sequence(Box::new(expr), Box::new(trailing_expr)),
            )
        }

        fn translate_assign(self_: &mut Ctx, trailing_expr: Expr, dst: &Place, expr: Expr) -> Expr {
            let expr = ptree_helpers::expr(
                Position::default(),
                ExprDesc::Assign(Box::new([(self_.translate_place_to_expr(dst), None, expr)])),
            );
            sequence_expr(trailing_expr, expr)
        }

        fn translate_assign_rvalue(
            self_: &mut Ctx,
            trailing_expr: Expr,
            place: &Place,
            rvalue: &Rvalue,
        ) -> Result<Expr> {
            let rvalue = self_.translate_rvalue_to_expr(rvalue)?;
            Ok(translate_assign(self_, trailing_expr, place, rvalue))
        }

        fn translate_copy_nonoverlapping(
            CopyNonOverlapping {
                src: _,
                dst: _,
                count: _,
            }: &CopyNonOverlapping,
        ) -> Expr {
            todo!()
        }

        fn translate_place_mention(self_: &mut Ctx, trailing_expr: Expr, place: &Place) -> Expr {
            let expr = self_.translate_place_to_expr(place);
            sequence_expr(trailing_expr, expr)
        }

        fn translate_abort(_: &AbortKind) -> Expr {
            ABSURD_EXPR.clone()
        }

        fn translate_assert(
            self_: &mut Ctx,
            trailing_expr: Expr,
            assert: &Assert,
            on_failure: &AbortKind,
        ) -> Result<Expr> {
            let cond = self_.translate_operand_to_expr(&assert.cond)?;
            let abort = translate_abort(on_failure);
            let expr = if assert.expected {
                ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::If(Box::new(cond), Box::new(UNIT_EXPR.clone()), Box::new(abort)),
                )
            } else {
                ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::If(Box::new(cond), Box::new(abort), Box::new(UNIT_EXPR.clone())),
                )
            };
            Ok(sequence_expr(trailing_expr, expr))
        }

        fn translate_call(
            self_: &mut Ctx,
            trailing_expr: Expr,
            Call {
                func,
                args,
                dest: dst,
            }: &Call,
        ) -> Result<Expr> {
            let func = match func {
                FnOperand::Regular(func_ref) => self_.translate_func_ref_to_expr(func_ref),
                FnOperand::Dynamic(operand) => self_.translate_operand_to_expr(operand)?,
            };
            let args: Vec<_> = args
                .iter()
                .map(|arg| self_.translate_operand_to_expr(arg))
                .try_collect()?;
            let expr = self_.translate_apply_expr(func, args);
            Ok(translate_assign(self_, trailing_expr, dst, expr))
        }

        let translate_return = || {
            ptree_helpers::expr(
                Position::default(),
                ExprDesc::Raise(
                    RETURN_LABEL.clone(),
                    Some(Box::new(ptree_helpers::evar(
                        Position::default(),
                        Qualid(Box::new([self.get_local(LocalId::ZERO).clone()])),
                    ))),
                ),
            )
        };
        let translate_break = |level| {
            ptree_helpers::expr(
                Position::default(),
                ExprDesc::Raise(
                    Qualid(Box::new([break_exn_ident(self.loop_depth - level)])),
                    None,
                ),
            )
        };
        let translate_continue = |level| {
            ptree_helpers::expr(
                Position::default(),
                ExprDesc::Raise(
                    Qualid(Box::new([continue_exn_ident(self.loop_depth - level)])),
                    None,
                ),
            )
        };

        fn translate_switch(self_: &mut Ctx, trailing_expr: Expr, switch: &Switch) -> Result<Expr> {
            fn translate_if(
                self_: &mut Ctx,
                cond: &Operand,
                block_t: &Block,
                block_f: &Block,
            ) -> Result<Expr> {
                Ok(ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::If(
                        Box::new(self_.translate_operand_to_expr(cond)?),
                        Box::new(self_.translate_block_to_expr(block_t)?),
                        Box::new(self_.translate_block_to_expr(block_f)?),
                    ),
                ))
            }

            fn translate_match(
                self_: &mut Ctx,
                scrutinee: &Place,
                arms: &[(Vec<VariantId>, Block)],
                otherwise: &Option<Block>,
            ) -> Result<Expr> {
                let scrutinee_ident = local_temp_ident(0);

                fn translate_discriminants_cond(
                    scrutinee: &Ident,
                    discriminants: &[VariantId],
                ) -> Expr {
                    let discriminant = discriminants.last().unwrap();
                    let expr = ptree_helpers::expr(
                        Position::default(),
                        ExprDesc::Infix(
                            Box::new(ptree_helpers::evar(
                                Position::default(),
                                Qualid(Box::new([scrutinee.clone()])),
                            )),
                            EQUAL.clone(),
                            Box::new(ptree_helpers::econst(
                                Position::default(),
                                discriminant.raw() as isize,
                            )),
                        ),
                    );
                    if discriminants.len() == 1 {
                        expr
                    } else {
                        ptree_helpers::expr(
                            Position::default(),
                            ExprDesc::Or(
                                Box::new(expr),
                                Box::new(translate_discriminants_cond(
                                    scrutinee,
                                    &discriminants[..discriminants.len() - 1],
                                )),
                            ),
                        )
                    }
                }

                let mut expr = match otherwise {
                    Some(block) => self_.translate_block_to_expr(block)?,
                    None => ABSURD_EXPR.clone(),
                };
                for (discriminants, block) in arms.iter().rev() {
                    expr = ptree_helpers::expr(
                        Position::default(),
                        ExprDesc::If(
                            Box::new(translate_discriminants_cond(
                                &scrutinee_ident,
                                discriminants,
                            )),
                            Box::new(self_.translate_block_to_expr(block)?),
                            Box::new(expr),
                        ),
                    );
                }
                Ok(ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::Let(
                        scrutinee_ident,
                        false,
                        RsKind::None,
                        Box::new(self_.translate_place_to_expr(scrutinee)),
                        Box::new(expr),
                    ),
                ))
            }

            let expr = match switch {
                Switch::If(cond, block_t, block_f) => translate_if(self_, cond, block_t, block_f)?,
                Switch::SwitchInt(..) => unreachable!(),
                Switch::Match(scrutinee, arms, otherwise) => {
                    translate_match(self_, scrutinee, arms, otherwise)?
                }
            };
            Ok(sequence_expr(trailing_expr, expr))
        }

        fn translate_loop(self_: &mut Ctx, trailing_expr: Expr, block: &Block) -> Result<Expr> {
            fn surround_exn(exn_ident: Ident, expr: Expr) -> Expr {
                ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::Exn(
                        exn_ident.clone(),
                        UNIT_TYPE.clone(),
                        Mask::Visible,
                        Box::new(ptree_helpers::expr(
                            Position::default(),
                            ExprDesc::Match(
                                Box::new(expr),
                                Box::new([]),
                                Box::new([(
                                    Qualid(Box::new([exn_ident])),
                                    None,
                                    UNIT_EXPR.clone(),
                                )]),
                            ),
                        )),
                    ),
                )
            }

            let expr = self_.enter_loop(|self_| {
                Ok(surround_exn(
                    break_exn_ident(self_.loop_depth),
                    ptree_helpers::expr(
                        Position::default(),
                        ExprDesc::While(
                            Box::new(TRUE_EXPR.clone()),
                            Box::new([]),
                            Box::new([]),
                            Box::new(surround_exn(
                                continue_exn_ident(self_.loop_depth),
                                self_.translate_block_to_expr(block)?,
                            )),
                        ),
                    ),
                ))
            })?;
            Ok(sequence_expr(trailing_expr, expr))
        }

        fn translate_contract_assert(
            self_: &mut Ctx,
            trailing_expr: Expr,
            kind: ContractAssertKind,
            spec_closure_id: SpecClosureId,
        ) -> Result<Expr> {
            let expr = ptree_helpers::expr(
                Position::default(),
                ExprDesc::Assert(
                    match kind {
                        ContractAssertKind::Assert => AssertionKind::Assert,
                        ContractAssertKind::Assume => AssertionKind::Assume,
                    },
                    self_.translate_spec_closure(
                        &self_.crate_.spec_closures[spec_closure_id],
                        &self_.name_map.spec_closure_names[spec_closure_id],
                    )?,
                ),
            );
            Ok(sequence_expr(trailing_expr, expr))
        }

        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                translate_assign_rvalue(self, trailing_expr, place, rvalue)
            }
            StatementKind::SetDiscriminant(..) => unreachable!(),
            StatementKind::CopyNonOverlapping(copy_nonoverlapping) => {
                Ok(translate_copy_nonoverlapping(copy_nonoverlapping))
            }
            StatementKind::StorageLive(..) | StatementKind::StorageDead(..) => Ok(trailing_expr),
            StatementKind::PlaceMention(place) => {
                Ok(translate_place_mention(self, trailing_expr, place))
            }
            StatementKind::Drop { .. } => Ok(trailing_expr), // Hack
            StatementKind::Assert {
                assert,
                on_failure,
                on_unwind: _,
            } => translate_assert(self, trailing_expr, assert, on_failure),
            StatementKind::InlineAsm { .. } => Err(Error::InlineAsm),
            StatementKind::Call { call, on_unwind: _ } => translate_call(self, trailing_expr, call),
            StatementKind::Abort(abort_kind) => Ok(translate_abort(abort_kind)),
            StatementKind::Return => Ok(translate_return()),
            StatementKind::UnwindResume => unreachable!(),
            StatementKind::Break(level) => Ok(translate_break(*level)),
            StatementKind::Continue(level) => Ok(translate_continue(*level)),
            StatementKind::Nop => unreachable!(),
            StatementKind::Switch(switch) => translate_switch(self, trailing_expr, switch),
            StatementKind::Loop(block) => translate_loop(self, trailing_expr, block),
            StatementKind::ContractAssert {
                kind,
                spec_closure_id,
            } => translate_contract_assert(self, trailing_expr, *kind, *spec_closure_id),
            StatementKind::Quant {
                kind: _,
                spec_closure_id: _,
                dest: _,
            } => unreachable!(),
            StatementKind::Error(..) => unreachable!(),
        }
    }
}
