use proc_macro2::TokenStream;
use syn::{Expr, ItemFn, parse_macro_input, parse2};

pub fn precondition(attr: TokenStream, item: TokenStream) -> TokenStream {
    // let attr = parse_macro_input!(attr as Expr);
    // let item = parse_macro_input!(item as ItemFn);
    item
}

pub fn postcondition(attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[cfg(test)]
mod tests {
    use super::*;

    use quote::quote;

    #[test]
    fn simple() {
        let item = quote! {
            fn square(x: u8) -> u8 {
                x * x
            }
        };
        let result = precondition(
            quote! {
                x < 16
            },
            item.clone(),
        );
        assert_eq!(
            parse2::<ItemFn>(result).unwrap(),
            parse2::<ItemFn>(item).unwrap(),
        );
    }
}
