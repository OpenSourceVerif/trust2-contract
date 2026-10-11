//! 局部变量（含规格闭包中的局部变量）的名字生成。

use charon_lib::{
    ast::{Body, FunSig, SpecClosure, SpecClosureId},
    ids::IndexMap,
    llbc_ast::{Block, StatementKind, Switch},
};

use std::collections::HashMap;

use super::{
    LocalNames,
    keywords::{TAKEN_WORDS_SET, ensure_l, to_whyml},
};

/// 为函数体（或其规格闭包）中的局部变量生成名字。
pub(super) struct LocalMapper<'a> {
    pub(super) spec_closures: &'a IndexMap<SpecClosureId, SpecClosure>,
    pub(super) spec_closure_names_opt: &'a mut IndexMap<SpecClosureId, Option<LocalNames>>,
}

impl<'a> LocalMapper<'a> {
    /// 为函数体（或规格闭包体）中的局部变量生成名字。
    pub(super) fn map(
        &mut self,
        body: &Body,
        signature_opt: Option<&FunSig>,
        depth: usize,
    ) -> LocalNames {
        match body {
            Body::Unstructured(_body) => unreachable!(),
            Body::Structured(body) => {
                self.traverse_spec_closures(&body.body, depth + 1);

                let locals = &body.locals.locals;

                let mut inv_map: HashMap<_, Vec<_>> = HashMap::new();
                let mut local_names = locals.map_ref_indexed(|local_id, local| {
                    let local_name = to_whyml(&ensure_l(format!("{local}").into()));
                    inv_map.entry(local_name).or_default().push(local_id);

                    String::default()
                });
                resolve_collision(inv_map, depth, |local_id, local_name| {
                    local_names[local_id] = local_name;
                });
                LocalNames::Concrete(local_names)
            }
            Body::TargetDispatch(..) => todo!(),
            Body::Extern(..) | Body::Opaque | Body::Missing => {
                LocalNames::Abstract(vec![None; signature_opt.unwrap().inputs.len()])
            }
            Body::Intrinsic { arg_names, .. } => LocalNames::Abstract(
                arg_names
                    .iter()
                    .map(|param_name_opt| {
                        param_name_opt
                            .as_ref()
                            .map(|param_name| to_whyml(&ensure_l(param_name.into())))
                    })
                    .collect(),
            ),
            Body::Error(..) => unreachable!(),
        }
    }

    /// 遍历语句，为其中出现的规格闭包生成局部变量名。
    fn traverse_spec_closures(&mut self, block: &Block, depth: usize) {
        for stmt in &block.statements {
            match &stmt.kind {
                StatementKind::InlineAsm { .. } => todo!(),
                StatementKind::Switch(Switch::If(_, block_t, block_f)) => {
                    self.traverse_spec_closures(block_t, depth);
                    self.traverse_spec_closures(block_f, depth);
                }
                StatementKind::Switch(Switch::SwitchInt(..)) => unreachable!(),
                StatementKind::Switch(Switch::Match(_, blocks, block_opt)) => {
                    for (_, block) in blocks {
                        self.traverse_spec_closures(block, depth);
                    }
                    block_opt
                        .iter()
                        .for_each(|block| self.traverse_spec_closures(block, depth));
                }
                StatementKind::Loop(block) => self.traverse_spec_closures(block, depth),
                StatementKind::ContractAssert {
                    spec_closure_id, ..
                }
                | StatementKind::Quant {
                    spec_closure_id, ..
                } => {
                    self.spec_closure_names_opt[*spec_closure_id] =
                        Some(self.map(&self.spec_closures[*spec_closure_id].body, None, depth));
                }
                _ => (),
            }
        }
    }
}

/// 为每个条目确定最终名字：与关键字冲突或有重名时，
/// 加上序号与深度后缀以消解冲突。
pub(super) fn resolve_collision<I: Copy>(
    inv_map: HashMap<String, Vec<I>>,
    depth: usize,
    mut set_map: impl FnMut(I, String),
) {
    for (name, ids) in inv_map {
        if !TAKEN_WORDS_SET.contains(name.as_str()) && ids.len() == 1 {
            let id = ids[0];
            set_map(id, format!("{name}'{depth}"));
        } else {
            for (disambiguator, id) in ids.into_iter().enumerate() {
                set_map(id, format!("{name}'{disambiguator}'{depth}"));
            }
        }
    }
}
