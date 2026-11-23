//! 把语句翻译成逻辑项（`Term`）。
//!
//! 规格中出现的函数体（例如后置条件里对函数体的引用）需要翻译成 WhyML 的项，
//! 此时不能用副作用和 `while`，因此控制流被表示为 `let` 绑定与嵌套的 `if`，
//! 每个基本块被翻译成一个以「用到的局部变量」为参数的 lambda。

use super::{
    Ctx,
    locals::{operand_local, operands_locals, rvalues_locals},
};

use itertools::Itertools;

use crate::llbc_to_whyml::{
    error::{Error, Result},
    ident::{EQUAL, local_block_ident, local_temp_ident, translate_ident},
    name_map::LocalNames,
    prelude::FALSE_TERM,
};

use charon_lib::{
    ast::{
        AbortKind, Assert, Call, FnOperand, LocalId, Operand, Place, PlaceKind, QuantKind, Rvalue,
        SpecClosureId, VariantId,
    },
    llbc_ast::{Block, Statement, StatementKind, Switch},
};

use why3_ptree::{
    dterm::{Dbinop, Dquant},
    loc::Position,
    ptree::{Binder, Ident, Qualid, Term, TermDesc},
    ptree_helpers,
};

use std::{
    collections::{BTreeSet, HashSet},
    iter,
};

impl<'a> Ctx<'a> {
    /// 翻译函数体最外层的基本块（供规格引用函数体时使用）。
    ///
    /// 每个基本块被绑定成一个 lambda，块之间的跳转变成对这些 lambda 的调用，
    /// `in_locals` 记录当前还「活着」（需要作为参数传递）的局部变量。
    pub(super) fn translate_outermost_block_to_term(&mut self, block: &Block) -> Result<Term> {
        let blocks: Vec<_> = block
            .statements
            .split_inclusive(|stmt| matches!(stmt.kind, StatementKind::Switch(..)))
            .enumerate()
            .collect();
        let mut blocks_rev = blocks.into_iter().rev();

        let mut in_locals_ = BTreeSet::new();
        let in_locals = &mut in_locals_;

        let terms_rev: Vec<_> = iter::once({
            let (block_id, stmts) = blocks_rev.next().unwrap();
            let mut stmts_rev = stmts.iter().rev();
            let stmt = stmts_rev.next().unwrap();
            assert!(stmt.kind.is_return());
            let mut term = self.translate_return_to_term(in_locals);
            for stmt in stmts_rev {
                term = self.translate_statement_to_term(stmt, term, in_locals)?;
            }
            Ok((block_id, term, in_locals.clone()))
        })
        .chain(blocks_rev.map(|(block_id, stmts)| {
            let mut term = self.translate_apply_term(
                ptree_helpers::tvar(
                    Position::default(),
                    Qualid(Box::new([local_block_ident(block_id + 1)])),
                ),
                in_locals.iter().copied().map(|local_id| {
                    ptree_helpers::tvar(
                        Position::default(),
                        Qualid(Box::new([self.get_local(local_id).clone()])),
                    )
                }),
            );
            for stmt in stmts.iter().rev() {
                term = self.translate_statement_to_term(stmt, term, in_locals)?;
            }
            Ok((block_id, term, in_locals.clone()))
        }))
        .try_collect()?;
        let mut terms = terms_rev.into_iter().rev();

        let (_, first_term, _) = terms.next().unwrap();
        Ok(
            terms.fold(first_term, |prev_term, (block_id, term, in_locals)| {
                ptree_helpers::term(
                    Position::default(),
                    TermDesc::Let(
                        local_block_ident(block_id),
                        Box::new(ptree_helpers::term(
                            Position::default(),
                            TermDesc::Quant(
                                Dquant::Lambda,
                                in_locals
                                    .into_iter()
                                    .map(|local_id| {
                                        Binder(
                                            Position::default(),
                                            Some(self.get_local(local_id).clone()),
                                            false,
                                            None,
                                        )
                                    })
                                    .collect(),
                                Box::new([]),
                                Box::new(term),
                            ),
                        )),
                        Box::new(prev_term),
                    ),
                )
            }),
        )
    }

    /// 翻译内层基本块：与 `translate_outermost_block_to_term` 类似，
    /// 但块的正常结束时继续执行 `continuation`。
    fn translate_inner_block_to_term(
        &mut self,
        block: &Block,
        continuation: Term,
        in_locals: &mut BTreeSet<LocalId>,
    ) -> Result<Term> {
        let blocks: Vec<_> = block
            .statements
            .split_inclusive(|stmt| matches!(stmt.kind, StatementKind::Switch(..)))
            .enumerate()
            .collect();
        let mut blocks_rev = blocks.into_iter().rev();

        let terms_rev: Vec<_> = iter::once({
            let (block_id, stmts) = blocks_rev.next().unwrap();
            let mut term = continuation;
            for stmt in stmts.iter().rev() {
                term = self.translate_statement_to_term(stmt, term, in_locals)?;
            }
            Ok((block_id, term, in_locals.clone()))
        })
        .chain(blocks_rev.map(|(block_id, stmts)| {
            let mut term = self.translate_apply_term(
                ptree_helpers::tvar(
                    Position::default(),
                    Qualid(Box::new([local_block_ident(block_id + 1)])),
                ),
                in_locals.iter().copied().map(|local_id| {
                    ptree_helpers::tvar(
                        Position::default(),
                        Qualid(Box::new([self.get_local(local_id).clone()])),
                    )
                }),
            );
            for stmt in stmts.iter().rev() {
                term = self.translate_statement_to_term(stmt, term, in_locals)?;
            }
            Ok((block_id, term, in_locals.clone()))
        }))
        .try_collect()?;
        let mut terms = terms_rev.into_iter().rev();

        let (_, first_term, _) = terms.next().unwrap();
        Ok(
            terms.fold(first_term, |prev_term, (block_id, term, in_locals)| {
                ptree_helpers::term(
                    Position::default(),
                    TermDesc::Let(
                        local_block_ident(block_id),
                        Box::new(ptree_helpers::term(
                            Position::default(),
                            TermDesc::Quant(
                                Dquant::Lambda,
                                in_locals
                                    .into_iter()
                                    .map(|local_id| {
                                        Binder(
                                            Position::default(),
                                            Some(self.get_local(local_id).clone()),
                                            false,
                                            None,
                                        )
                                    })
                                    .collect(),
                                Box::new([]),
                                Box::new(term),
                            ),
                        )),
                        Box::new(prev_term),
                    ),
                )
            }),
        )
    }

    /// 翻译 `return`：记录返回值的局部变量并产生返回值对应的项。
    fn translate_return_to_term(&self, in_locals: &mut BTreeSet<LocalId>) -> Term {
        in_locals.insert(LocalId::ZERO);
        ptree_helpers::tvar(
            Position::default(),
            Qualid(Box::new([self.get_local(LocalId::ZERO).clone()])),
        )
    }

    /// 把一条语句翻译成项，`trailing_term` 是其后继语句翻译出的项。
    fn translate_statement_to_term(
        &mut self,
        stmt: &Statement,
        trailing_term: Term,
        in_locals: &mut BTreeSet<LocalId>,
    ) -> Result<Term> {
        fn translate_assign(
            self_: &mut Ctx,
            trailing_term: Term,
            dst: &Place,
            term: Term,
            in_locals: &mut BTreeSet<LocalId>,
            update_in_locals: impl FnOnce(&mut BTreeSet<LocalId>),
        ) -> Term {
            // TODO
            let PlaceKind::Local(local_id) = dst.kind else {
                todo!();
            };
            in_locals.remove(&local_id);
            update_in_locals(in_locals);
            ptree_helpers::term(
                Position::default(),
                TermDesc::Let(
                    self_.get_local(local_id).clone(),
                    Box::new(term),
                    Box::new(trailing_term),
                ),
            )
        }

        fn translate_assign_rvalue(
            self_: &mut Ctx,
            trailing_term: Term,
            place: &Place,
            rvalue: &Rvalue,
            in_locals: &mut BTreeSet<LocalId>,
        ) -> Result<Term> {
            let locals_set = rvalues_locals([rvalue])?;
            let rvalue = self_.translate_rvalue_to_term(rvalue)?;
            Ok(translate_assign(
                self_,
                trailing_term,
                place,
                rvalue,
                in_locals,
                |in_locals| in_locals.extend(locals_set),
            ))
        }

        fn translate_abort(_: &AbortKind) -> Term {
            FALSE_TERM.clone()
        }

        fn translate_assert(
            self_: &mut Ctx,
            trailing_term: Term,
            assert: &Assert,
            on_failure: &AbortKind,
            in_locals: &mut BTreeSet<LocalId>,
        ) -> Result<Term> {
            in_locals.extend(operand_local(&assert.cond));
            let cond = self_.translate_operand_to_term(&assert.cond)?;
            let abort = translate_abort(on_failure);
            Ok(if assert.expected {
                ptree_helpers::term(
                    Position::default(),
                    TermDesc::If(Box::new(cond), Box::new(trailing_term), Box::new(abort)),
                )
            } else {
                ptree_helpers::term(
                    Position::default(),
                    TermDesc::If(Box::new(cond), Box::new(abort), Box::new(trailing_term)),
                )
            })
        }

        fn translate_call(
            self_: &mut Ctx,
            trailing_term: Term,
            Call {
                func,
                args,
                dest: dst,
            }: &Call,
            in_locals: &mut BTreeSet<LocalId>,
        ) -> Result<Term> {
            let mut locals_set = HashSet::new();
            let func = match func {
                FnOperand::Regular(func_ref) => self_.translate_func_ref_to_term(func_ref),
                FnOperand::Dynamic(operand) => {
                    locals_set.extend(operand_local(operand));
                    self_.translate_operand_to_term(operand)?
                }
            };
            locals_set.extend(operands_locals(args));
            let args: Vec<_> = args
                .iter()
                .map(|arg| self_.translate_operand_to_term(arg))
                .try_collect()?;
            let term = self_.translate_apply_term(func, args);
            Ok(translate_assign(
                self_,
                trailing_term,
                dst,
                term,
                in_locals,
                |in_locals| in_locals.extend(locals_set),
            ))
        }

        fn translate_switch(
            self_: &mut Ctx,
            continuation: Term,
            switch: &Switch,
            in_locals: &mut BTreeSet<LocalId>,
        ) -> Result<Term> {
            fn translate_if(
                self_: &mut Ctx,
                cond: &Operand,
                block_t: &Block,
                block_f: &Block,
                continuation: Term,
                in_locals: &mut BTreeSet<LocalId>,
            ) -> Result<Term> {
                let mut in_locals_ = in_locals.clone();
                let term_t = self_.translate_inner_block_to_term(
                    block_t,
                    continuation.clone(),
                    &mut in_locals_,
                )?;
                let term_f =
                    self_.translate_inner_block_to_term(block_f, continuation, in_locals)?;
                in_locals.extend(in_locals_);
                in_locals.extend(operand_local(cond));
                Ok(ptree_helpers::term(
                    Position::default(),
                    TermDesc::If(
                        Box::new(self_.translate_operand_to_term(cond)?),
                        Box::new(term_t),
                        Box::new(term_f),
                    ),
                ))
            }

            fn translate_match(
                self_: &mut Ctx,
                scrutinee: &Place,
                arms: &[(Vec<VariantId>, Block)],
                otherwise: &Option<Block>,
                continuation: Term,
                in_locals: &mut BTreeSet<LocalId>,
            ) -> Result<Term> {
                let scrutinee_ident = local_temp_ident(0);

                fn translate_discriminants_cond(
                    scrutinee: &Ident,
                    discriminants: &[VariantId],
                ) -> Term {
                    let discriminant = discriminants.last().unwrap();
                    let term = ptree_helpers::term(
                        Position::default(),
                        TermDesc::Infix(
                            Box::new(ptree_helpers::tvar(
                                Position::default(),
                                Qualid(Box::new([scrutinee.clone()])),
                            )),
                            EQUAL.clone(),
                            Box::new(ptree_helpers::tconst(
                                Position::default(),
                                discriminant.raw() as isize,
                            )),
                        ),
                    );
                    if discriminants.len() == 1 {
                        term
                    } else {
                        ptree_helpers::term(
                            Position::default(),
                            TermDesc::Binop(
                                Box::new(term),
                                Dbinop::Or,
                                Box::new(translate_discriminants_cond(
                                    scrutinee,
                                    &discriminants[..discriminants.len() - 1],
                                )),
                            ),
                        )
                    }
                }

                let in_locals_old = in_locals.clone();
                let mut term = match otherwise {
                    Some(block) => self_.translate_inner_block_to_term(
                        block,
                        continuation.clone(),
                        in_locals,
                    )?,
                    None => FALSE_TERM.clone(),
                };
                for (discriminants, block) in arms.iter().rev() {
                    let mut in_locals_ = in_locals_old.clone();
                    let term_t = self_.translate_inner_block_to_term(
                        block,
                        continuation.clone(),
                        &mut in_locals_,
                    )?;
                    in_locals.extend(in_locals_);
                    term = ptree_helpers::term(
                        Position::default(),
                        TermDesc::If(
                            Box::new(translate_discriminants_cond(
                                &scrutinee_ident,
                                discriminants,
                            )),
                            Box::new(term_t),
                            Box::new(term),
                        ),
                    );
                }
                in_locals.extend(scrutinee.local_id());
                Ok(ptree_helpers::term(
                    Position::default(),
                    TermDesc::Let(
                        scrutinee_ident,
                        Box::new(self_.translate_place_to_term(scrutinee)),
                        Box::new(term),
                    ),
                ))
            }

            match switch {
                Switch::If(cond, block_t, block_f) => {
                    translate_if(self_, cond, block_t, block_f, continuation, in_locals)
                }
                Switch::SwitchInt(..) => unreachable!(),
                Switch::Match(scrutinee, arms, otherwise) => {
                    translate_match(self_, scrutinee, arms, otherwise, continuation, in_locals)
                }
            }
        }

        fn translate_quant(
            self_: &mut Ctx,
            trailing_term: Term,
            kind: QuantKind,
            spec_closure_id: SpecClosureId,
            dst: &Place,
            in_locals: &mut BTreeSet<LocalId>,
        ) -> Result<Term> {
            let spec_closure = &self_.crate_.spec_closures[spec_closure_id];
            let locals = &spec_closure.body.locals().locals;
            let local_names = &self_.name_map.spec_closure_names[spec_closure_id];
            let LocalNames::Concrete(local_names_) = local_names else {
                unreachable!();
            };

            let locals_set = rvalues_locals(&spec_closure.captures)?;
            let term = ptree_helpers::term(
                Position::default(),
                TermDesc::Quant(
                    match kind {
                        QuantKind::ForAll => Dquant::Forall,
                        QuantKind::Exists => Dquant::Exists,
                    },
                    spec_closure
                        .non_captured_argument_ids()
                        .map(|local_id| {
                            Ok(Binder(
                                Position::default(),
                                Some(translate_ident(local_names_[local_id].as_str().into())),
                                false,
                                Some(self_.translate_type(&locals[local_id].ty)?),
                            ))
                        })
                        .try_collect()?,
                    Box::new([]),
                    Box::new(self_.translate_spec_closure(spec_closure, local_names)?),
                ),
            );
            Ok(translate_assign(
                self_,
                trailing_term,
                dst,
                term,
                in_locals,
                |in_locals| {
                    in_locals.extend(locals_set);
                },
            ))
        }

        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                translate_assign_rvalue(self, trailing_term, place, rvalue, in_locals)
            }
            StatementKind::SetDiscriminant(..) => unreachable!(),
            StatementKind::CopyNonOverlapping(..) => Err(Error::CopyNonOverlappingInSpec),
            StatementKind::StorageLive(..) | StatementKind::StorageDead(..) => Ok(trailing_term),
            StatementKind::PlaceMention(_place) => Ok(trailing_term),
            StatementKind::Drop { .. } => unreachable!(),
            StatementKind::Assert {
                assert,
                on_failure,
                on_unwind: _,
            } => translate_assert(self, trailing_term, assert, on_failure, in_locals),
            StatementKind::InlineAsm { .. } => Err(Error::InlineAsm),
            StatementKind::Call { call, on_unwind: _ } => {
                translate_call(self, trailing_term, call, in_locals)
            }
            StatementKind::Abort(abort_kind) => Ok(translate_abort(abort_kind)),
            StatementKind::Return => Ok(self.translate_return_to_term(in_locals)),
            StatementKind::UnwindResume => unreachable!(),
            StatementKind::Break(..) | StatementKind::Continue(..) | StatementKind::Loop(..) => {
                Err(Error::LoopInSpec)
            }
            StatementKind::Nop => unreachable!(),
            StatementKind::Switch(switch) => {
                translate_switch(self, trailing_term, switch, in_locals)
            }
            StatementKind::ContractAssert { .. } => Err(Error::NestedSpec),
            StatementKind::Quant {
                kind,
                spec_closure_id,
                dest,
            } => translate_quant(
                self,
                trailing_term,
                *kind,
                *spec_closure_id,
                dest,
                in_locals,
            ),
            StatementKind::Error(..) => unreachable!(),
        }
    }
}
