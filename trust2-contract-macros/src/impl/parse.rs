use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;

use std::iter;

pub fn rewrite_implications(old_tokens: TokenStream) -> TokenStream {
    let mut new_tokens = Vec::new();

    for token in old_tokens {
        new_tokens.push(match token {
            TokenTree::Group(group) => TokenTree::Group(Group::new(
                group.delimiter(),
                rewrite_implications(group.stream()),
            )),
            _ => token,
        });
    }

    if let Some(closure_end) = split_closure(&new_tokens) {
        let mut closure = new_tokens.drain(..=closure_end).collect::<TokenStream>();
        closure.extend(rewrite_implications(new_tokens.into_iter().collect()));
        return closure;
    }

    if let Some((operator, operator_index, operator_len)) = find_implication_operator(&new_tokens) {
        let rhs_tokens = new_tokens.split_off(operator_index + operator_len);
        new_tokens.truncate(operator_index);
        let lhs = rewrite_implications(new_tokens.into_iter().collect());
        let rhs = rewrite_implications(rhs_tokens.into_iter().collect());
        return match operator {
            ImplicationOperator::LeftToRight => quote! {
                implies(#lhs, #rhs)
            },
            ImplicationOperator::RightToLeft => quote! {
                implies(#rhs, #lhs)
            },
            ImplicationOperator::Iff => quote! {
                (#lhs) == (#rhs)
            },
        };
    }

    new_tokens.into_iter().collect()
}

pub fn replace_keywords(
    old_tokens: TokenStream,
    keywords: &[&str],
    crate_name_ident: &Ident,
) -> TokenStream {
    let mut new_tokens = TokenStream::new();
    for token in old_tokens {
        let tokens: Box<dyn Iterator<Item = TokenTree>> = match token {
            TokenTree::Group(group) => Box::new(iter::once(TokenTree::Group(Group::new(
                group.delimiter(),
                replace_keywords(group.stream(), keywords, crate_name_ident),
            )))),
            TokenTree::Ident(ref ident) => {
                if keywords.contains(&ident.to_string().as_str()) {
                    Box::new(
                        quote! {
                            ::#crate_name_ident::internal::#ident
                        }
                        .into_iter(),
                    )
                } else {
                    Box::new(iter::once(token))
                }
            }
            _ => Box::new(iter::once(token)),
        };
        new_tokens.extend(tokens);
    }
    new_tokens
}

fn split_closure(tokens: &[TokenTree]) -> Option<usize> {
    let start = match tokens.first() {
        Some(TokenTree::Punct(punct)) if punct.as_char() == '|' => 0,
        Some(TokenTree::Ident(ident)) if ident == "move" => match tokens.get(1) {
            Some(TokenTree::Punct(punct)) if punct.as_char() == '|' => 1,
            _ => return None,
        },
        _ => return None,
    };

    tokens
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, token)| match token {
            TokenTree::Punct(punct) if punct.as_char() == '|' => Some(index),
            _ => None,
        })
}

#[derive(Clone, Copy)]
enum ImplicationOperator {
    LeftToRight,
    RightToLeft,
    Iff,
}

fn find_implication_operator(tokens: &[TokenTree]) -> Option<(ImplicationOperator, usize, usize)> {
    let mut index = 0;
    while index < tokens.len() {
        if punct_seq_matches(&tokens[index..], &['<', '=', '=', '>']) {
            return Some((ImplicationOperator::Iff, index, 4));
        }
        if punct_seq_matches(&tokens[index..], &['=', '=', '>']) {
            return Some((ImplicationOperator::LeftToRight, index, 3));
        }
        if punct_seq_matches(&tokens[index..], &['<', '=', '=']) {
            return Some((ImplicationOperator::RightToLeft, index, 3));
        }
        index += 1;
    }
    None
}

fn punct_seq_matches(tokens: &[TokenTree], chars: &[char]) -> bool {
    tokens.len() >= chars.len()
        && tokens
            .iter()
            .zip(chars)
            .all(|(token, ch)| matches!(token, TokenTree::Punct(punct) if punct.as_char() == *ch))
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
    fn test_implies_left_to_right() {
        let expr = quote! {
            a ==> b
        };
        let expect = quote! {
            implies(a, b)
        };
        let result = rewrite_implications(expr);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap(),
        );
    }

    #[test]
    fn test_implies_right_to_left() {
        let expr = quote! {
            a <== b
        };
        let expect = quote! {
            implies(b, a)
        };
        let result = rewrite_implications(expr);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap(),
        );
    }

    #[test]
    fn test_iff() {
        let expr = quote! {
            a <==> b
        };
        let expect = quote! {
            (a) == (b)
        };
        let result = rewrite_implications(expr);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap(),
        );
    }

    #[test]
    fn test_closure_body() {
        let expr = quote! {
            |result| result ==> x == 0
        };
        let expect = quote! {
            |result| implies(result, x == 0)
        };
        let result = rewrite_implications(expr);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap(),
        );
    }

    #[test]
    fn test_typed_closure_body() {
        let expr = quote! {
            |i: usize| i < len ==> xs[i] == 0
        };
        let expect = quote! {
            |i: usize| implies(i < len, xs[i] == 0)
        };
        let result = rewrite_implications(expr);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap(),
        );
    }

}
