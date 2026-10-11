//! `Expr` 与 `Term` 两种输出共用的构造器抽象。
//!
//! 同一份翻译逻辑既要产出可执行代码（`Expr`），也要产出规格中的项（`Term`）。
//! [`Builder`] 把这两种目标中构造语法树的操作抽象出来，翻译逻辑只需针对
//! `T::T` 编写，通过泛型参数选择 [`ExprBuilder`] 或 [`TermBuilder`] 即可。

use super::prelude::{FALSE_EXPR, FALSE_TERM, TRUE_EXPR, TRUE_TERM, UNIT_EXPR, UNIT_TERM};

use why3_ptree::{
    constant::Constant,
    loc::Position,
    ptree::{Expr, ExprDesc, Ident, Qualid, Term, TermDesc},
    ptree_helpers,
};

pub trait Builder {
    type T;

    /// 由常量构造语法树节点。
    fn const_(loc: Position, c: Constant) -> Self::T;

    /// 由限定名构造变量引用。
    fn var(loc: Position, id: Qualid) -> Self::T;

    /// 函数应用 `f l1 l2 ...`。
    fn app(loc: Position, f: Qualid, l: Box<[Self::T]>) -> Self::T;

    /// 单参数应用 `e1 e2`。
    fn apply(loc: Position, e1: Self::T, e2: Self::T) -> Self::T;

    /// 中缀运算 `e1 <id> e2`。
    fn infix(loc: Position, e1: Self::T, id: Ident, e2: Self::T) -> Self::T;

    /// 元组构造。
    fn tuple(loc: Position, es: Box<[Self::T]>) -> Self::T;

    /// 记录构造。
    fn record(loc: Position, fs: Box<[(Qualid, Self::T)]>) -> Self::T;

    /// 作用域限定 `id: e`（用于把整数运算限定到对应类型）。
    fn scope(loc: Position, id: Qualid, e: Self::T) -> Self::T;

    /// `true`。
    fn true_() -> Self::T;

    /// `false`。
    fn false_() -> Self::T;

    /// 单元值 `()`。
    fn unit_value() -> Self::T;
}

pub struct ExprBuilder;

impl Builder for ExprBuilder {
    type T = Expr;

    fn const_(loc: Position, c: Constant) -> Self::T {
        ptree_helpers::expr(loc, ExprDesc::Const(c))
    }

    fn var(loc: Position, id: Qualid) -> Self::T {
        ptree_helpers::evar(loc, id)
    }

    fn app(loc: Position, f: Qualid, l: Box<[Self::T]>) -> Self::T {
        ptree_helpers::eapp(loc, f, l)
    }

    fn apply(loc: Position, e1: Self::T, e2: Self::T) -> Self::T {
        ptree_helpers::eapply(loc, e1, e2)
    }

    fn infix(loc: Position, e1: Self::T, id: Ident, e2: Self::T) -> Self::T {
        ptree_helpers::expr(loc, ExprDesc::Infix(Box::new(e1), id, Box::new(e2)))
    }

    fn tuple(loc: Position, es: Box<[Self::T]>) -> Self::T {
        ptree_helpers::expr(loc, ExprDesc::Tuple(es))
    }

    fn record(loc: Position, fs: Box<[(Qualid, Self::T)]>) -> Self::T {
        ptree_helpers::expr(loc, ExprDesc::Record(fs))
    }

    fn scope(loc: Position, id: Qualid, e: Self::T) -> Self::T {
        ptree_helpers::expr(loc, ExprDesc::Scope(id, Box::new(e)))
    }

    fn true_() -> Self::T {
        TRUE_EXPR.clone()
    }

    fn false_() -> Self::T {
        FALSE_EXPR.clone()
    }

    fn unit_value() -> Self::T {
        UNIT_EXPR.clone()
    }
}

pub struct TermBuilder;

impl Builder for TermBuilder {
    type T = Term;

    fn const_(loc: Position, c: Constant) -> Self::T {
        ptree_helpers::term(loc, TermDesc::Const(c))
    }

    fn var(loc: Position, id: Qualid) -> Self::T {
        ptree_helpers::tvar(loc, id)
    }

    fn app(loc: Position, f: Qualid, l: Box<[Self::T]>) -> Self::T {
        ptree_helpers::tapp(loc, f, l)
    }

    fn apply(loc: Position, e1: Self::T, e2: Self::T) -> Self::T {
        ptree_helpers::term(loc, TermDesc::Apply(Box::new(e1), Box::new(e2)))
    }

    fn infix(loc: Position, e1: Self::T, id: Ident, e2: Self::T) -> Self::T {
        ptree_helpers::term(loc, TermDesc::Infix(Box::new(e1), id, Box::new(e2)))
    }

    fn tuple(loc: Position, es: Box<[Self::T]>) -> Self::T {
        ptree_helpers::term(loc, TermDesc::Tuple(es))
    }

    fn record(loc: Position, fs: Box<[(Qualid, Self::T)]>) -> Self::T {
        ptree_helpers::term(loc, TermDesc::Record(fs))
    }

    fn scope(loc: Position, id: Qualid, e: Self::T) -> Self::T {
        ptree_helpers::term(loc, TermDesc::Scope(id, Box::new(e)))
    }

    fn true_() -> Self::T {
        TRUE_TERM.clone()
    }

    fn false_() -> Self::T {
        FALSE_TERM.clone()
    }

    fn unit_value() -> Self::T {
        UNIT_TERM.clone()
    }
}
