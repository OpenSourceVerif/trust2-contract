use proc_macro2::{Delimiter, Group, Ident, TokenStream, TokenTree};
use quote::quote;

use std::iter;

pub fn rewrite_implications(old_tokens: TokenStream) -> TokenStream {
    let tokens: Vec<_> = rewrite_groups_and_implies_calls(old_tokens)
        .into_iter()
        .collect();

    rewrite_implications_in_tokens(tokens)
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

fn rewrite_groups_and_implies_calls(old_tokens: TokenStream) -> TokenStream {
    let mut new_tokens = TokenStream::new();
    let mut tokens = old_tokens.into_iter().peekable();

    while let Some(token) = tokens.next() {
        if let TokenTree::Ident(ident) = &token
            && ident == "implies"
            && let Some(TokenTree::Group(group)) = tokens.peek()
            && group.delimiter() == Delimiter::Parenthesis
        {
            let group = match tokens.next().unwrap() {
                TokenTree::Group(group) => group,
                _ => unreachable!(),
            };
            let Some((lhs, rhs)) = split_top_level_comma(group.stream()) else {
                new_tokens.extend(iter::once(token));
                new_tokens.extend(iter::once(TokenTree::Group(group)));
                continue;
            };
            let lhs = rewrite_implications(lhs);
            let rhs = rewrite_implications(rhs);
            new_tokens.extend(quote! {
                if #lhs { #rhs } else { true }
            });
            continue;
        }

        match token {
            TokenTree::Group(group) => {
                new_tokens.extend(iter::once(TokenTree::Group(Group::new(
                    group.delimiter(),
                    rewrite_implications(group.stream()),
                ))));
            }
            _ => new_tokens.extend(iter::once(token)),
        }
    }

    new_tokens
}

fn rewrite_implications_in_tokens(tokens: Vec<TokenTree>) -> TokenStream {
    if let Some((closure_prefix, closure_body)) = split_closure(tokens.clone()) {
        let mut new_tokens = TokenStream::new();
        new_tokens.extend(closure_prefix);
        new_tokens.extend(rewrite_implications_in_tokens(closure_body));
        return new_tokens;
    }

    if let Some((operator, operator_index, operator_len)) = find_implication_operator(&tokens) {
        let lhs = rewrite_implications_in_tokens(tokens[..operator_index].to_vec());
        let rhs = rewrite_implications_in_tokens(tokens[operator_index + operator_len..].to_vec());
        return match operator {
            ImplicationOperator::LeftToRight => quote! {
                if #lhs { #rhs } else { true }
            },
            ImplicationOperator::RightToLeft => quote! {
                if #rhs { #lhs } else { true }
            },
            ImplicationOperator::Iff => quote! {
                (#lhs) == (#rhs)
            },
        };
    }

    tokens.into_iter().collect()
}

fn split_closure(tokens: Vec<TokenTree>) -> Option<(Vec<TokenTree>, Vec<TokenTree>)> {
    let start = match tokens.first() {
        Some(TokenTree::Punct(punct)) if punct.as_char() == '|' => 0,
        Some(TokenTree::Ident(ident)) if ident == "move" => match tokens.get(1) {
            Some(TokenTree::Punct(punct)) if punct.as_char() == '|' => 1,
            _ => return None,
        },
        _ => return None,
    };

    let end = tokens
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, token)| match token {
            TokenTree::Punct(punct) if punct.as_char() == '|' => Some(index),
            _ => None,
        })?;

    Some((tokens[..=end].to_vec(), tokens[end + 1..].to_vec()))
}

fn split_top_level_comma(tokens: TokenStream) -> Option<(TokenStream, TokenStream)> {
    let tokens: Vec<_> = tokens.into_iter().collect();
    let comma_index = tokens
        .iter()
        .position(|token| matches!(token, TokenTree::Punct(punct) if punct.as_char() == ','))?;

    Some((
        tokens[..comma_index].iter().cloned().collect(),
        tokens[comma_index + 1..].iter().cloned().collect(),
    ))
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
            if a { b } else { true }
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
            if b { a } else { true }
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
            |result| if result { x == 0 } else { true }
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
            |i: usize| if i < len { xs[i] == 0 } else { true }
        };
        let result = rewrite_implications(expr);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap(),
        );
    }

    #[test]
    fn test_implies_function_call() {
        let expr = quote! {
            implies(i < len, xs[i] == 0)
        };
        let expect = quote! {
            if i < len { xs[i] == 0 } else { true }
        };
        let result = rewrite_implications(expr);
        assert_eq!(
            parse::<Expr>(expect).unwrap(),
            parse::<Expr>(result).unwrap(),
        );
    }
}
