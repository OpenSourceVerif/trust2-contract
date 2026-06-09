use why3_ptree::{
    constant::Constant,
    loc::Position,
    ptree::{Expr, ExprDesc, Ident, Qualid, Term, TermDesc},
    ptree_helpers,
};

pub trait Builder {
    type T;

    fn const_(loc: Position, c: Constant) -> Self::T;

    fn var(loc: Position, id: Qualid) -> Self::T;

    fn app(loc: Position, f: Qualid, l: Box<[Self::T]>) -> Self::T;

    fn apply(loc: Position, e1: Self::T, e2: Self::T) -> Self::T;

    fn infix(loc: Position, e1: Self::T, id: Ident, e2: Self::T) -> Self::T;

    fn tuple(loc: Position, es: Box<[Self::T]>) -> Self::T;

    fn record(loc: Position, fs: Box<[(Qualid, Self::T)]>) -> Self::T;

    fn scope(loc: Position, id: Qualid, e: Self::T) -> Self::T;

    fn true_() -> Self::T;

    fn false_() -> Self::T;

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
        super::TRUE_EXPR.clone()
    }

    fn false_() -> Self::T {
        super::FALSE_EXPR.clone()
    }

    fn unit_value() -> Self::T {
        super::UNIT_EXPR.clone()
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
        super::TRUE_TERM.clone()
    }

    fn false_() -> Self::T {
        super::FALSE_TERM.clone()
    }

    fn unit_value() -> Self::T {
        super::UNIT_TERM.clone()
    }
}
