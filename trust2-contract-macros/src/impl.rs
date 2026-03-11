use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{Error, Expr, ItemEnum, ItemFn, ItemStruct, ItemUnion, ReturnType, parse_quote, parse2 as parse};

use std::sync::LazyLock;

mod parse;

macro_rules! parse_macro_input {
    ($tokenstream:ident as $ty:ty) => {
        match parse::<$ty>($tokenstream) {
            Ok(data) => data,
            Err(err) => return err.into_compile_error(),
        }
    };
}

pub fn precondition(expr: TokenStream, item: TokenStream) -> TokenStream {
    let crate_name = Ident::new(&CRATE_NAME, Span::mixed_site());
    let expr = parse::replace_keywords(expr, COMMON_KEYWORDS, &crate_name);
    let expr = parse_macro_input!(expr as Expr);
    let mut item = parse_macro_input!(item as ItemFn);

    let stmt = parse_quote! {
        {
            ::#crate_name::internal::entry();
            ::#crate_name::internal::precondition(|| #expr);
        }
    };
    item.block.stmts.insert(0, stmt);
    quote! {
        #item
    }
}

pub fn postcondition(expr: TokenStream, item: TokenStream) -> TokenStream {
    let crate_name = Ident::new(&CRATE_NAME, Span::mixed_site());
    let expr = parse::replace_keywords(expr, COMMON_KEYWORDS, &crate_name);
    let expr = parse::replace_keywords(expr, POSTCONDITION_KEYWORDS, &crate_name);
    let expr = parse_macro_input!(expr as Expr);
    let mut item = parse_macro_input!(item as ItemFn);

    let ty: TokenStream = match item.sig.output {
        ReturnType::Default => quote! {
            ()
        },
        ReturnType::Type(_, ref ty) => quote! {
            #ty
        },
    };
    let stmt = parse_quote! {
        {
            ::#crate_name::internal::entry();
            ::#crate_name::internal::postcondition::<#ty, _>(#expr);
        }
    };
    item.block.stmts.insert(0, stmt);
    quote! {
        #item
    }
}

pub fn invariant(expr: TokenStream, item: TokenStream) -> TokenStream {
    let crate_name = Ident::new(&CRATE_NAME, Span::mixed_site());
    let expr = parse::replace_keywords(expr, COMMON_KEYWORDS, &crate_name);
    let expr = parse_macro_input!(expr as Expr);
    let (type_ident, type_generics) = {
        let type_name = (|item: &TokenStream| {
            // if let Ok(item_type) = parse::<ItemType>(item.clone()) {
            //     return Ok((item_type.ident, item_type.generics));
            // }
            if let Ok(item_struct) = parse::<ItemStruct>(item.clone()) {
                return Ok((item_struct.ident, item_struct.generics));
            }
            if let Ok(item_enum) = parse::<ItemEnum>(item.clone()) {
                return Ok((item_enum.ident, item_enum.generics));
            }
            if let Ok(item_union) = parse::<ItemUnion>(item.clone()) {
                return Ok((item_union.ident, item_union.generics));
            }
            Err(Error::new(
                Span::mixed_site(),
                "expect a type declaration (struct, enum, or union)"
            ).into_compile_error())
        })(&item);
        match type_name {
            Ok(type_name) => type_name,
            Err(err) => return err,
        }
    };

    let (impl_generics, type_generics, where_clause) = type_generics.split_for_impl();
    quote! {
        #item

        impl #impl_generics ::#crate_name::internal::TypeInvariant for #type_ident #type_generics #where_clause {
            fn invariant(&self) -> ::std::primitive::bool {
                #expr
            }
        }
    }
}

pub fn contract_assert(expr: TokenStream) -> TokenStream {
    let crate_name = Ident::new(&CRATE_NAME, Span::mixed_site());
    let expr = parse::replace_keywords(expr, COMMON_KEYWORDS, &crate_name);
    let expr = parse_macro_input!(expr as Expr);

    quote! {
        {
            ::#crate_name::internal::entry();
            ::#crate_name::internal::contract_assert(|| #expr);
        }
    }
}

pub fn contract_assume(expr: TokenStream) -> TokenStream {
    let crate_name = Ident::new(&CRATE_NAME, Span::mixed_site());
    let expr = parse::replace_keywords(expr, COMMON_KEYWORDS, &crate_name);
    let expr = parse_macro_input!(expr as Expr);

    quote! {
        {
            ::#crate_name::internal::entry();
            ::#crate_name::internal::contract_assume(|| #expr);
        }
    }
}

#[cfg(not(test))]
static CRATE_NAME: LazyLock<String> = {
    use proc_macro_crate::FoundCrate;

    LazyLock::new(|| {
        match proc_macro_crate::crate_name("trust2-contract") {
            Ok(FoundCrate::Name(name)) => name,
            Ok(_) => unreachable!(),
            Err(_) => panic!(),
        }
    })
};

#[cfg(test)]
static CRATE_NAME: LazyLock<String> = LazyLock::new(|| "trust2_contract".into());

const COMMON_KEYWORDS: &[&str] = &["forall", "exists", "implies"];

const POSTCONDITION_KEYWORDS: &[&str] = &["old"];

#[cfg(test)]
mod tests {
    use super::*;

    use syn::File;

    #[test]
    fn test_precondition() {
        let item = quote! {
            fn square(x: u8) -> u8 {
                x * x
            }
        };
        let expr = quote! {
            x < 16
        };
        let expect = quote! {
            fn square(x: u8) -> u8 {
                {
                    ::trust2_contract::internal::entry();
                    ::trust2_contract::internal::precondition(|| #expr);
                }
                x * x
            }
        };
        let result = precondition(expr, item);
        assert_eq!(
            parse::<ItemFn>(expect).unwrap(),
            parse::<ItemFn>(result).unwrap(),
        );
    }

    #[test]
    fn test_invariant() {
        let item = quote! {
            struct RefRange<'a, T: PartialOrd> {
                start: &'a T,
                end: &'a T,
            }
        };
        let expr = quote! {
            self.start <= self.end
        };
        let expect = quote! {
            #item

            impl<'a, T: PartialOrd> ::trust2_contract::internal::TypeInvariant for RefRange<'a, T> {
                fn invariant(&self) -> ::std::primitive::bool {
                    #expr
                }
            }
        };
        let result = invariant(expr, item);
        assert_eq!(
            parse::<File>(expect).unwrap(),
            parse::<File>(result).unwrap(),
        );
    }

    #[test]
    fn test_postcondition_forall_implies() {
        let item = quote! {
            fn to_sorted(a: &[i32]) -> Vec<i32> {
                vec![]
            }
        };
        let expr = quote! {
            |b| forall(|i: usize| implies(i + 1 < a.len(), b[i] <= b[i + 1]))
        };
        let expect = quote! {
            fn to_sorted(a: &[i32]) -> Vec<i32> {
                {
                    ::trust2_contract::internal::entry();
                    ::trust2_contract::internal::postcondition::<Vec<i32>, _>(|b| ::trust2_contract::internal::forall(|i: usize| ::trust2_contract::internal::implies(i + 1 < a.len(), b[i] <= b[i + 1])));
                }
                vec![]
            }
        };
        let result = postcondition(expr, item);
        assert_eq!(
            parse::<ItemFn>(expect).unwrap(),
            parse::<ItemFn>(result).unwrap(),
        );
    }
}
