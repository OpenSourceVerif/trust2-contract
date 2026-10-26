use proc_macro2::{Delimiter, Group, Ident, TokenStream, TokenTree};
use quote::quote;

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
