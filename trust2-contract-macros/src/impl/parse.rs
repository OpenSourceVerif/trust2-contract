use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;

use std::iter;

pub fn replace_keywords(old_tokens: TokenStream, keywords: &[&str], crate_name: &Ident) -> TokenStream {
    let mut new_tokens = TokenStream::new();
    for token in old_tokens {
        let tokens: Box<dyn Iterator<Item = TokenTree>> = match token {
            TokenTree::Group(group) => Box::new(iter::once(TokenTree::Group(Group::new(
                group.delimiter(),
                replace_keywords(group.stream(), keywords, crate_name),
            )))),
            TokenTree::Ident(ref ident) => {
                if keywords.contains(&ident.to_string().as_str()) {
                    Box::new(quote! {
                        ::#crate_name::internal::#ident
                    }.into_iter())
                } else {
                    Box::new(iter::once(token))
                }
            },
            _ => Box::new(iter::once(token)),
        };
        new_tokens.extend(tokens);
    }
    new_tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::{COMMON_KEYWORDS, POSTCONDITION_KEYWORDS};

    use proc_macro2::Span;
    use syn::{Expr, parse2 as parse};

    #[test]
    fn test_exists() {
        let expr = quote! {
            exists(|x: usize| y == x * x)
        };
        let crate_name = Ident::new("trust2_contract", Span::mixed_site());
        let expect = quote! {
            ::trust2_contract::internal::exists(|x: usize| y == x * x)
        };
        let result = replace_keywords(expr, COMMON_KEYWORDS, &crate_name);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap(),
        );
    }

    #[test]
    fn test_old() {
        let expr = quote! {
            *y == *old(x)
        };
        let crate_name = Ident::new("trust2_contract", Span::mixed_site());
        let expect = quote! {
            *y == *::trust2_contract::internal::old(x)
        };
        let result = replace_keywords(expr, POSTCONDITION_KEYWORDS, &crate_name);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap(),
        );
    }
}
