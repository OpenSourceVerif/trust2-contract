use proc_macro2::{Delimiter, Group, Ident, Spacing, TokenStream, TokenTree};
use quote::quote;
use syn::{
    Error, Expr, ExprRange, RangeLimits, Result, Token, Type,
    parse::{Parse, ParseStream},
    parse_quote,
};

/// Rewrites every `a ==> b` to `!(a) || (b)`.
///
/// `==>` binds weaker than every other operator and associates to the right. It does not expand
/// to `implies(a, b)` because a call evaluates both arguments, whereas `||` evaluates `b` only
/// when `a` holds, which is what gives `i < v.len() ==> v[i] > 0` its meaning.
pub fn rewrite_implications(old_tokens: TokenStream) -> TokenStream {
    let tokens: Vec<_> = old_tokens
        .into_iter()
        .map(|token| match token {
            TokenTree::Group(group) => {
                let mut new_group =
                    Group::new(group.delimiter(), rewrite_implications(group.stream()));
                new_group.set_span(group.span());
                TokenTree::Group(new_group)
            }
            token => token,
        })
        .collect();

    let mut new_tokens = TokenStream::new();
    let mut rest = &tokens[..];
    while !rest.is_empty() {
        let (prefix, after_prefix) = rest.split_at(implication_prefix_len(rest));
        new_tokens.extend(prefix.iter().cloned());
        let (operands, separator_len) = split_at_separator(after_prefix);
        new_tokens.extend(rewrite_operands(operands));
        let after_operands = &after_prefix[operands.len()..];
        let (separator, after_separator) = after_operands.split_at(separator_len);
        new_tokens.extend(separator.iter().cloned());
        rest = after_separator;
    }
    new_tokens
}

fn rewrite_operands(tokens: &[TokenTree]) -> TokenStream {
    match (0..tokens.len()).find(|&index| is_implication(&tokens[index..])) {
        Some(index) => {
            let lhs: TokenStream = tokens[..index].iter().cloned().collect();
            let rhs = rewrite_operands(&tokens[index + 3..]);
            quote! { !(#lhs) || (#rhs) }
        }
        None => tokens.iter().cloned().collect(),
    }
}

/// The length of the leading `|x|`, `let x =` and `return`, which are not part of the antecedent.
fn implication_prefix_len(tokens: &[TokenTree]) -> usize {
    let mut len = 0;
    loop {
        let rest = &tokens[len..];
        let skipped = match rest.first() {
            Some(TokenTree::Ident(ident)) if ident == "return" => Some(1),
            Some(TokenTree::Ident(ident)) if ident == "move" => Some(1),
            Some(TokenTree::Ident(ident)) if ident == "let" => rest
                .iter()
                .enumerate()
                .take_while(|(_, token)| !is_punct(token, ';'))
                .find(|&(index, token)| {
                    is_punct(token, '=')
                        && !is_joint(token)
                        && !rest.get(index.wrapping_sub(1)).is_some_and(is_joint)
                })
                .map(|(index, _)| index + 1),
            Some(token) if is_punct(token, '|') => rest
                .iter()
                .skip(1)
                .position(|token| is_punct(token, '|'))
                .map(|index| index + 2),
            _ => None,
        };
        match skipped {
            Some(skipped) => len += skipped,
            None => return len,
        }
    }
}

/// Operands end at `,`, at `;` and at the `=>` of a match arm. Returns the tokens up to the
/// separator and the length of the separator.
fn split_at_separator(tokens: &[TokenTree]) -> (&[TokenTree], usize) {
    let mut index = 0;
    while index < tokens.len() {
        if is_implication(&tokens[index..]) {
            index += 3;
        } else if is_punct(&tokens[index], ',') || is_punct(&tokens[index], ';') {
            return (&tokens[..index], 1);
        } else if is_punct(&tokens[index], '=')
            && is_joint(&tokens[index])
            && tokens.get(index + 1).is_some_and(|token| is_punct(token, '>'))
        {
            return (&tokens[..index], 2);
        } else {
            index += 1;
        }
    }
    (tokens, 0)
}

/// Tokens forwarded through `macro_rules!` may lose the jointness of the second `=`. `== >` is
/// never valid Rust, so ignoring it cannot change the meaning of a valid program.
fn is_implication(tokens: &[TokenTree]) -> bool {
    matches!(
        tokens,
        [first, second, third, ..]
            if is_punct(first, '=') && is_joint(first)
                && is_punct(second, '=')
                && is_punct(third, '>')
    )
}

fn is_punct(token: &TokenTree, ch: char) -> bool {
    matches!(token, TokenTree::Punct(punct) if punct.as_char() == ch)
}

fn is_joint(token: &TokenTree) -> bool {
    matches!(token, TokenTree::Punct(punct) if punct.spacing() == Spacing::Joint)
}

/// The argument of `each!`: `i: T; guard; body`, or `i: T in lo..hi; body`.
pub struct Each {
    pub var: Ident,
    pub ty: Type,
    pub guard: Expr,
    pub body: Expr,
}

impl Parse for Each {
    fn parse(input: ParseStream) -> Result<Self> {
        let var: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty = input.parse()?;
        let guard = if input.parse::<Option<Token![in]>>()?.is_some() {
            let range: ExprRange = input.parse()?;
            let (Some(lo), Some(hi)) = (&range.start, &range.end) else {
                return Err(Error::new_spanned(
                    &range,
                    "the range of `each!` needs both bounds, as in `0..n`",
                ));
            };
            match range.limits {
                RangeLimits::HalfOpen(_) => parse_quote! { (#lo) <= #var && #var < (#hi) },
                RangeLimits::Closed(_) => parse_quote! { (#lo) <= #var && #var <= (#hi) },
            }
        } else {
            input.parse::<Token![;]>()?;
            input.parse()?
        };
        input.parse::<Token![;]>()?;
        let body = input.parse()?;
        Ok(Self {
            var,
            ty,
            guard,
            body,
        })
    }
}

/// Rewrites every `old(e)` to `old(&(e))`.
///
/// `old` only accepts a mutable reference. Passing it directly would make the
/// postcondition closure capture it by unique borrow, which is neither `Fn` nor `Copy`.
pub fn borrow_old_arguments(old_tokens: TokenStream) -> TokenStream {
    let mut new_tokens = TokenStream::new();
    let mut tokens = old_tokens.into_iter().peekable();
    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Group(group) => {
                let mut new_group =
                    Group::new(group.delimiter(), borrow_old_arguments(group.stream()));
                new_group.set_span(group.span());
                new_tokens.extend([TokenTree::Group(new_group)]);
            }
            TokenTree::Ident(ref ident) if ident == "old" => {
                new_tokens.extend([token]);
                if let Some(TokenTree::Group(group)) = tokens.peek()
                    && group.delimiter() == Delimiter::Parenthesis
                {
                    let arg = borrow_old_arguments(group.stream());
                    let mut new_group = Group::new(Delimiter::Parenthesis, quote! { &(#arg) });
                    new_group.set_span(group.span());
                    new_tokens.extend([TokenTree::Group(new_group)]);
                    tokens.next();
                }
            }
            token => new_tokens.extend([token]),
        }
    }
    new_tokens
}

pub fn replace_keywords(
    old_tokens: TokenStream,
    keywords: &[&str],
    crate_name_ident: &Ident,
) -> TokenStream {
    let mut new_tokens = TokenStream::new();
    for token in old_tokens {
        match token {
            TokenTree::Group(group) => {
                new_tokens.extend([TokenTree::Group(Group::new(
                    group.delimiter(),
                    replace_keywords(group.stream(), keywords, crate_name_ident),
                ))]);
            }
            TokenTree::Ident(ref ident) if keywords.contains(&ident.to_string().as_str()) => {
                new_tokens.extend(quote! { ::#crate_name_ident::internal::#ident });
            }
            token => new_tokens.extend([token]),
        }
    }
    new_tokens
}

#[cfg(test)]
mod tests {
    use super::super::{POSTCONDITION_KEYWORDS, crate_name_ident};
    use super::*;

    use syn::{Expr, parse2 as parse};

    #[test]
    fn test_old() {
        let expr = quote! {
            *y == *old(x)
        };
        let crate_name_ident = crate_name_ident();
        let expect = quote! {
            *y == *::#crate_name_ident::internal::old(&(x))
        };
        let result = borrow_old_arguments(expr);
        let result = replace_keywords(result, POSTCONDITION_KEYWORDS, &crate_name_ident);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap(),
        );
    }

    #[test]
    fn borrows_old_arguments_in_nested_groups() {
        let expr = quote! {
            forall!(|i: usize| implies(i < old(v).len(), old(v)[i] == f(old_value(x), old))) && old
        };
        let expect = quote! {
            forall!(|i: usize| implies(i < old(&(v)).len(), old(&(v))[i] == f(old_value(x), old))) && old
        };
        let result = borrow_old_arguments(expr);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap(),
        );
    }

    fn assert_implications_rewritten(expr: TokenStream, expect: TokenStream) {
        assert_eq!(rewrite_implications(expr).to_string(), expect.to_string());
    }

    #[test]
    fn implication_binds_weaker_than_other_operators() {
        assert_implications_rewritten(
            quote! { a && b || c ==> d == e },
            quote! { !(a && b || c) || (d == e) },
        );
    }

    #[test]
    fn implication_associates_to_the_right() {
        assert_implications_rewritten(
            quote! { a ==> b ==> c },
            quote! { !(a) || (!(b) || (c)) },
        );
    }

    #[test]
    fn implication_stops_at_closure_headers_and_lets() {
        assert_implications_rewritten(quote! { |r| a ==> b }, quote! { |r| !(a) || (b) });
        assert_implications_rewritten(quote! { move |r| a ==> b }, quote! { move |r| !(a) || (b) });
        assert_implications_rewritten(quote! { || a ==> b }, quote! { || !(a) || (b) });
        assert_implications_rewritten(
            quote! { { let y: u8 = a ==> b; y ==> c } },
            quote! { { let y: u8 = !(a) || (b); !(y) || (c) } },
        );
    }

    #[test]
    fn implication_stops_at_separators() {
        assert_implications_rewritten(
            quote! { f(a ==> b, c ==> d) },
            quote! { f(!(a) || (b), !(c) || (d)) },
        );
        assert_implications_rewritten(
            quote! { match x { A => a ==> b, _ => c } },
            quote! { match x { A => !(a) || (b), _ => c } },
        );
        assert_implications_rewritten(
            quote! { forall!(|i: u8| |x, y| a ==> b) },
            quote! { forall!(|i: u8| |x, y| !(a) || (b)) },
        );
    }

    #[test]
    fn other_operators_are_not_implications() {
        let expr = quote! { a >= b && c <= d && e == f && g != h };
        assert_implications_rewritten(expr.clone(), expr);
        let arm = quote! { match x { A => y, B => z } };
        assert_implications_rewritten(arm.clone(), arm);
    }

    #[test]
    fn each_with_guard() {
        let each: Each = parse(quote! { i: usize; i < n; a[i] > 0 }).unwrap();
        assert_eq!(each.var, "i");
        assert_eq!(each.guard, parse_quote! { i < n });
        assert_eq!(each.body, parse_quote! { a[i] > 0 });
    }

    #[test]
    fn each_with_range() {
        let each: Each = parse(quote! { i: u32 in lo..hi; a }).unwrap();
        assert_eq!(each.guard, parse_quote! { (lo) <= i && i < (hi) });
        let each: Each = parse(quote! { i: u32 in 0..=9; a }).unwrap();
        assert_eq!(each.guard, parse_quote! { (0) <= i && i <= (9) });
    }

    #[test]
    fn each_rejects_malformed_arguments() {
        for each in [
            quote! { i: u32 in 0..; a },
            quote! { i: u32 in ..9; a },
            quote! { i: u32 in 0; a },
            quote! { i; i < 9; a },
            quote! { i: u32; a },
            quote! { i: u32; i < 9; a; b },
        ] {
            assert!(parse::<Each>(each).is_err());
        }
    }

    #[test]
    fn replaces_keywords_in_nested_groups() {
        let crate_name_ident = crate_name_ident();
        let expr = quote! {
            implies(flag, { [old(x), old(y)][0] == old((x + y)) })
        };
        let expect = quote! {
            ::#crate_name_ident::internal::implies(flag, {
                [::#crate_name_ident::internal::old(x), ::#crate_name_ident::internal::old(y)][0]
                    == ::#crate_name_ident::internal::old((x + y))
            })
        };
        let result = replace_keywords(expr, &["implies", "old"], &crate_name_ident);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap()
        );
    }

    #[test]
    fn preserves_other_tokens() {
        let expr = quote! { (old_value, "old", 'x', 42, [x + y], { f(x) }) };
        let result = replace_keywords(expr.clone(), &["old"], &crate_name_ident());
        assert_eq!(parse::<Expr>(expr).unwrap(), parse::<Expr>(result).unwrap());
    }
}
