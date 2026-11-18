use proc_macro2::{Group, Ident, Spacing, TokenStream, TokenTree};
use quote::quote;

/// Tracks preceding punctuation so member accesses (`x.implies`) and path segments (`a::implies`)
/// are never rewritten as free specification keywords.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PrevPunct {
    /// No relevant trailing punctuation.
    None,
    /// Saw a `Joint` `.` (start of `..` or `..=`), so the next `.` is not a member access dot.
    DotPrefix,
    /// Saw a standalone `.` introducing a field or method name.
    MemberDot,
    /// Saw the first `:` of a `::` path separator (`Spacing::Joint`).
    JointColon,
    /// Saw a complete `::` path separator.
    PathSep,
}

/// Replaces free occurrences of `keywords` in `old_tokens` with
/// `::#crate_name_ident::internal::<keyword>`, recursing into token groups while preserving
/// qualified path segments (`::keyword`) and member accesses (`.keyword`).
pub fn replace_keywords(
    old_tokens: TokenStream,
    keywords: &[&str],
    crate_name_ident: &Ident,
) -> TokenStream {
    let mut new_tokens = TokenStream::new();
    let mut prev = PrevPunct::None;
    for token in old_tokens {
        match token {
            TokenTree::Group(group) => {
                prev = PrevPunct::None;
                new_tokens.extend([TokenTree::Group(Group::new(
                    group.delimiter(),
                    replace_keywords(group.stream(), keywords, crate_name_ident),
                ))]);
            }
            TokenTree::Ident(ref ident)
                if !matches!(prev, PrevPunct::MemberDot | PrevPunct::PathSep)
                    && keywords.contains(&ident.to_string().as_str()) =>
            {
                prev = PrevPunct::None;
                new_tokens.extend(quote! { ::#crate_name_ident::internal::#ident });
            }
            TokenTree::Punct(ref punct) => {
                prev = match punct.as_char() {
                    '.' => match prev {
                        PrevPunct::DotPrefix => {
                            if punct.spacing() == Spacing::Joint {
                                PrevPunct::DotPrefix
                            } else {
                                PrevPunct::None
                            }
                        }
                        _ if punct.spacing() == Spacing::Joint => PrevPunct::DotPrefix,
                        _ => PrevPunct::MemberDot,
                    },
                    ':' => match prev {
                        PrevPunct::JointColon => PrevPunct::PathSep,
                        _ if punct.spacing() == Spacing::Joint => PrevPunct::JointColon,
                        _ => PrevPunct::None,
                    },
                    _ => PrevPunct::None,
                };
                new_tokens.extend([token]);
            }
            token => {
                prev = PrevPunct::None;
                new_tokens.extend([token]);
            }
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
    fn skips_qualified_paths_and_member_accesses_and_is_idempotent() {
        let crate_name_ident = crate_name_ident();
        let expr = quote! {
            (
                x.implies(y),
                x.old,
                foo::implies(a, b),
                ::#crate_name_ident::internal::implies(a, b),
                S { flag: implies(a, b) },
                0..old(n),
            )
        };
        let expect = quote! {
            (
                x.implies(y),
                x.old,
                foo::implies(a, b),
                ::#crate_name_ident::internal::implies(a, b),
                S { flag: ::#crate_name_ident::internal::implies(a, b) },
                0..::#crate_name_ident::internal::old(n),
            )
        };
        let once = replace_keywords(expr, &["implies", "old"], &crate_name_ident);
        let twice = replace_keywords(once.clone(), &["implies", "old"], &crate_name_ident);
        let expect_expr = parse::<Expr>(expect).unwrap();
        assert_eq!(expect_expr, parse::<Expr>(once).unwrap());
        assert_eq!(expect_expr, parse::<Expr>(twice).unwrap());
    }

    #[test]
    fn preserves_other_tokens() {
        let expr = quote! { (old_value, "old", 'x', 42, [x + y], { f(x) }) };
        let result = replace_keywords(expr.clone(), &["old"], &crate_name_ident());
        assert_eq!(parse::<Expr>(expr).unwrap(), parse::<Expr>(result).unwrap());
    }
}
