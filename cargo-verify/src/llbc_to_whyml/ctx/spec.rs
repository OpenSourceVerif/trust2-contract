//! 规格闭包（spec closure）的翻译。
//!
//! 前置条件、后置条件、契约断言与量词在 LLBC 中都是「闭包 + 捕获的局部变量」，
//! 这里把它们统一翻译成一个 WhyML 项：先把捕获的变量用 `let` 绑定，
//! 再翻译闭包体。

use super::Ctx;

use crate::llbc_to_whyml::{error::Result, ident::translate_ident, name_map::LocalNames};

use charon_lib::ast::SpecClosure;

use why3_ptree::{
    loc::Position,
    ptree::{Term, TermDesc},
    ptree_helpers,
};

impl<'a> Ctx<'a> {
    /// 翻译规格闭包，把捕获的变量绑定在闭包体外面。
    /// 调用前必须已经用 [`Ctx::with_locals`] 设置好局部变量名。
    pub(super) fn translate_spec_closure(
        &mut self,
        spec_closure: &SpecClosure,
        local_names: &LocalNames,
    ) -> Result<Term> {
        let LocalNames::Concrete(local_names) = local_names else {
            unreachable!();
        };

        let term = self.with_locals(local_names, |self_| {
            let body = spec_closure.body.as_structured().unwrap();
            self_.translate_outermost_block_to_term(&body.body)
        })?;
        spec_closure
            .captures
            .iter_enumerated()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .try_fold(term, |term, (local_id, rvalue)| {
                Ok(ptree_helpers::term(
                    Position::default(),
                    TermDesc::Let(
                        translate_ident(local_names[local_id].as_str().into()),
                        Box::new(self.translate_rvalue_to_term(rvalue)?),
                        Box::new(term),
                    ),
                ))
            })
    }
}
