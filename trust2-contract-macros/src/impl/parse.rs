use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;

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
            *y == *::#crate_name_ident::internal::old(x)
        };
        let result = replace_keywords(expr, POSTCONDITION_KEYWORDS, &crate_name_ident);
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
