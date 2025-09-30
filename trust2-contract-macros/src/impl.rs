use proc_macro_crate::FoundCrate;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Expr, ItemFn, parse_quote, parse2 as parse, ReturnType, spanned::Spanned};

use std::sync::LazyLock;

macro_rules! parse_macro_input {
    ($tokenstream:ident as $ty:ty) => {
        match parse::<$ty>($tokenstream) {
            Ok(data) => data,
            Err(err) => return err.into_compile_error().into(),
        }
    };
}

pub fn precondition(expr: TokenStream, item: TokenStream) -> TokenStream {
    let expr = parse_macro_input!(expr as Expr);
    let mut item = parse_macro_input!(item as ItemFn);

    let crate_name = Ident::new(&CRATE_NAME, Span::call_site());
    let stmt = parse_quote! {
        ::#crate_name::internal::precondition(|| #expr);
    };
    item.block.stmts.insert(0, stmt);
    quote! {
        #item
    }
}

pub fn postcondition(expr: TokenStream, item: TokenStream) -> TokenStream {
    let expr = parse_macro_input!(expr as Expr);
    let mut item = parse_macro_input!(item as ItemFn);

    let crate_name = Ident::new(&CRATE_NAME, Span::call_site());
    let ty: TokenStream = match item.sig.output {
        ReturnType::Default => quote! {
            ()
        },
        ReturnType::Type(_, ref ty) => quote_spanned! {ty.span() =>
            #ty
        },
    };
    let stmt = parse_quote! {
        ::#crate_name::internal::postcondition::<#ty, _>(#expr);
    };
    item.block.stmts.insert(0, stmt);
    quote! {
        #item
    }
}

static CRATE_NAME: LazyLock<String> = LazyLock::new(|| {
    match proc_macro_crate::crate_name("trust2-contract") {
        Ok(FoundCrate::Name(name)) => name,
        Ok(_) => unreachable!(),
        Err(_) => "trust2_contract".into(),
    }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
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
                ::trust2_contract::internal::precondition(|| #expr);
                x * x
            }
        };
        let result = precondition(expr, item);
        assert_eq!(
            parse::<ItemFn>(result).unwrap(),
            parse::<ItemFn>(expect).unwrap(),
        );
    }
}
