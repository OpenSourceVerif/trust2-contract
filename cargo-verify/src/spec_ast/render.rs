//! Rendering helpers and pretty-printer for [`Spec`].
//!
//! Should we put this in under `pretty::` instead? It is currently under `ast::spec_ast`.

use super::{
    BinOp, Binder, LiteralConst, Pattern, PatternDesc, Qualid, Quant, Spec, Term, TermDesc,
};
use std::fmt;

impl fmt::Display for Spec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.pre.is_empty() {
            writeln!(f, "pre: []")?;
        } else {
            writeln!(f, "pre:")?;
            for term in &self.pre {
                writeln!(f, "  - {}", term.as_pretty())?;
            }
        }
        if self.post.is_empty() {
            writeln!(f, "post: []")?;
        } else {
            writeln!(f, "post:")?;
            for post in &self.post {
                writeln!(
                    f,
                    "  - {} => {}",
                    post.pattern.as_pretty(),
                    post.term.as_pretty()
                )?;
            }
        }
        Ok(())
    }
}

impl Pattern {
    fn as_pretty(&self) -> String {
        match &self.desc {
            PatternDesc::Wild => "_".to_owned(),
            PatternDesc::Var(id) => id.name.clone(),
            PatternDesc::Tuple(fields) => {
                let fields = fields.iter().map(|f| f.as_pretty()).collect::<Vec<_>>();
                format!("({})", fields.join(", "))
            }
        }
    }
}

impl Term {
    fn as_pretty(&self) -> String {
        match &self.desc {
            TermDesc::True => "true".to_owned(),
            TermDesc::False => "false".to_owned(),
            TermDesc::Const(c) => match c {
                LiteralConst::Bool(v) => v.to_string(),
                LiteralConst::Int(v) => format!("{v:?}"),
                LiteralConst::Char(v) => format!("{v:?}"),
                LiteralConst::Str(v) => format!("{v:?}"),
                LiteralConst::Unit => "()".to_owned(),
            },
            TermDesc::Ident(qid) => format_qualid(qid),
            TermDesc::IdApp(qid, args) => {
                let args = args.iter().map(Term::as_pretty).collect::<Vec<_>>();
                format!("{}({})", format_qualid(qid), args.join(", "))
            }
            TermDesc::Apply(lhs, rhs) => format!("({} {})", lhs.as_pretty(), rhs.as_pretty()),
            TermDesc::Infix(lhs, op, rhs) => {
                format!("({} {} {})", lhs.as_pretty(), op.name, rhs.as_pretty())
            }
            TermDesc::BinOp(lhs, op, rhs) => {
                format!(
                    "({} {} {})",
                    lhs.as_pretty(),
                    format_binop(*op),
                    rhs.as_pretty()
                )
            }
            TermDesc::Not(t) => format!("not ({})", t.as_pretty()),
            TermDesc::If(c, t, e) => {
                format!(
                    "(if {} then {} else {})",
                    c.as_pretty(),
                    t.as_pretty(),
                    e.as_pretty()
                )
            }
            TermDesc::Quant(q, binders, _triggers, body) => {
                let q = format_quant(*q);
                let binders = format_binders(binders);
                format!("{q} {binders}. {}", body.as_pretty())
            }
            // TermDesc::Attr(_, term) => term.as_pretty(),
            TermDesc::Let(id, rhs, body) => {
                format!(
                    "(let {} = {} in {})",
                    id.name,
                    rhs.as_pretty(),
                    body.as_pretty()
                )
            }
            TermDesc::Case(scrutinee, branches) => {
                let branches = branches
                    .iter()
                    .map(|(pat, term)| format!("{} => {}", pat.as_pretty(), term.as_pretty()))
                    .collect::<Vec<_>>();
                format!(
                    "(case {} of {})",
                    scrutinee.as_pretty(),
                    branches.join(" | ")
                )
            }
            TermDesc::Cast(term, _) => format!("cast({})", term.as_pretty()),
            TermDesc::Tuple(fields) => {
                let fields = fields.iter().map(Term::as_pretty).collect::<Vec<_>>();
                format!("({})", fields.join(", "))
            }
            TermDesc::Record(fields) => {
                let fields = fields
                    .iter()
                    .map(|(qid, term)| format!("{} = {}", format_qualid(qid), term.as_pretty()))
                    .collect::<Vec<_>>();
                format!("{{{}}}", fields.join(", "))
            }
            TermDesc::Update(base, updates) => {
                let updates = updates
                    .iter()
                    .map(|(qid, term)| format!("{} = {}", format_qualid(qid), term.as_pretty()))
                    .collect::<Vec<_>>();
                format!("{{{} with {}}}", base.as_pretty(), updates.join(", "))
            }
            TermDesc::Scope(scope, term) => {
                format!("{}::{}", format_qualid(scope), term.as_pretty())
            }
            TermDesc::At(term, label) => format!("at({}, {})", term.as_pretty(), label.name),
        }
    }
}

fn format_qualid(qid: &Qualid) -> String {
    match qid {
        Qualid::Ident(id) => id.name.clone(),
        Qualid::Dot(parent, id) => format!("{}.{}", format_qualid(parent), id.name),
    }
}

fn format_quant(quant: Quant) -> &'static str {
    match quant {
        Quant::Forall => "forall",
        Quant::Exists => "exists",
    }
}

fn format_binders(binders: &[Binder]) -> String {
    binders
        .iter()
        .map(|binder| {
            binder
                .id
                .as_ref()
                .map(|id| id.name.clone())
                .unwrap_or_else(|| "_".to_owned())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_binop(op: BinOp) -> &'static str {
    match op {
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::Implies => "->",
        BinOp::Iff => "<->",
        BinOp::Eq => "=",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        BinOp, Binder, Ident, Pattern, PatternDesc, Post, Qualid, Quant, Spec, Term, TermDesc,
    };
    use charon_lib::ast::Span;

    #[test]
    fn pretty_snapshot_post_nested_quantifier_old() {
        let span = Span::dummy();
        let i = Ident::new("i", span);
        let j = Ident::new("j", span);
        let result = Ident::new("result", span);

        let result_term = Term::new(span, TermDesc::Ident(Qualid::Ident(result.clone())));
        let old_result = Term::new(
            span,
            TermDesc::At(Box::new(result_term.clone()), Ident::new("old", span)),
        );
        let i_term = Term::new(span, TermDesc::Ident(Qualid::Ident(i.clone())));
        let j_term = Term::new(span, TermDesc::Ident(Qualid::Ident(j.clone())));

        let nested = Term::new(
            span,
            TermDesc::Quant(
                Quant::Forall,
                vec![Binder {
                    span,
                    id: Some(j.clone()),
                    ghost: false,
                    ty: None,
                }],
                vec![],
                Box::new(Term::new(
                    span,
                    TermDesc::BinOp(Box::new(j_term), BinOp::Lt, Box::new(i_term.clone())),
                )),
            ),
        );
        let post_term = Term::new(
            span,
            TermDesc::BinOp(Box::new(old_result), BinOp::And, Box::new(nested)),
        );
        let spec = Spec {
            pre: Vec::new(),
            post: vec![Post {
                span,
                pattern: Pattern {
                    span,
                    desc: PatternDesc::Var(result),
                },
                term: post_term,
            }],
        };

        let got = spec.to_string();
        let expected = "\
pre: []
post:
  - result => (at(result, old) && forall j. (j < i))
";
        assert_eq!(got, expected);
    }
}
