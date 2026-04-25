//! Why3-style specification AST used as the lowered semantic form of `spec`.
//!
//! The structures in this module intentionally mirror the naming and layering of Why3's
//! `ptree.ml`.

use charon_lib::ast::{ScalarValue, Span, Ty};

mod render;

/// Identifier node (`ident` in Why3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    /// Human-readable identifier text.
    pub name: String,
    /// Source span of this identifier.
    pub span: Span,
    // pub attrs: Vec<Attr>,
}

impl Ident {
    /// Build an identifier with no attributes.
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span,
        }
    }
}

/// Qualified identifier (`qualid` in Why3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Qualid {
    /// Single-segment identifier.
    Ident(Ident),
    /// Dotted path: `left.right`.
    Dot(Box<Qualid>, Ident),
}

impl Qualid {
    /// Build a single-segment qualified identifier.
    pub fn ident(name: impl Into<String>, span: Span) -> Self {
        Self::Ident(Ident::new(name, span))
    }
}

/// Ghost flag (`ghost` in Why3 binders).
pub type Ghost = bool;

/// Binder node: `(loc, id option, ghost, ty option)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binder {
    /// Source span.
    pub span: Span,
    /// Optional bound identifier.
    pub id: Option<Ident>,
    /// Ghost marker.
    pub ghost: Ghost,
    /// Optional type annotation.
    pub ty: Option<Ty>,
}

/// Pattern node (`pattern` in Why3), currently a V1 subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// Source span.
    pub span: Span,
    /// Pattern payload.
    pub desc: PatternDesc,
}

/// Pattern forms supported by V1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternDesc {
    /// Wildcard `_`.
    Wild,
    /// Variable binder.
    Var(Ident),
    /// Tuple pattern.
    Tuple(Vec<Pattern>),
}

/// Logical term node (`term` in Why3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    /// Source span.
    pub span: Span,
    /// Term payload.
    pub desc: TermDesc,
}

impl Term {
    /// Build a term from a span and descriptor.
    pub fn new(span: Span, desc: TermDesc) -> Self {
        Self { span, desc }
    }
}

/// Core subset of Why3 `term_desc`.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermDesc {
    True,
    False,
    Const(LiteralConst),
    Ident(Qualid),
    IdApp(Qualid, Vec<Term>),
    Apply(Box<Term>, Box<Term>),
    Infix(Box<Term>, Ident, Box<Term>),
    BinOp(Box<Term>, BinOp, Box<Term>),
    Not(Box<Term>),
    If(Box<Term>, Box<Term>, Box<Term>),
    Quant(Quant, Vec<Binder>, Vec<Vec<Term>>, Box<Term>),
    // Attr(Attr, Box<Term>),
    Let(Ident, Box<Term>, Box<Term>),
    Case(Box<Term>, Vec<(Pattern, Term)>),
    Cast(Box<Term>, Ty),
    Tuple(Vec<Term>),
    Record(Vec<(Qualid, Term)>),
    Update(Box<Term>, Vec<(Qualid, Term)>),
    Scope(Qualid, Box<Term>),
    /// `old` lowering target.
    At(Box<Term>, Ident),
}

/// Quantifier kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quant {
    Forall,
    Exists,
}

/// Binary operator kind.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    And,
    Or,
    Implies,
    Iff,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

/// Literal constants supported by V1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteralConst {
    Bool(bool),
    Int(ScalarValue),
    Char(char),
    Str(Box<str>),
    Unit,
}

/// Full lowered specification.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Spec {
    /// Precondition terms.
    pub pre: Vec<Term>,
    /// Postcondition entries.
    pub post: Vec<Post>,
}

/// Postcondition entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Post {
    /// Source span of the originating postcondition call.
    pub span: Span,
    /// Result pattern bound by the postcondition closure.
    pub pattern: Pattern,
    /// Logical term attached to `pattern`.
    pub term: Term,
}
