use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn precondition(attr: TokenStream, item: TokenStream) -> TokenStream {
    inner::precondition(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn postcondition(attr: TokenStream, item: TokenStream) -> TokenStream {
    inner::postcondition(attr.into(), item.into()).into()
}

mod inner {
    use proc_macro2::TokenStream;

    pub fn precondition(attr: TokenStream, item: TokenStream) -> TokenStream {
        // let attr = parse_macro_input!(attr as syn::Expr);
        // let item = parse_macro_input!(item as syn::ItemFn);
        item
    }

    pub fn postcondition(attr: TokenStream, item: TokenStream) -> TokenStream {
        item
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use quote::quote;
    use syn::{ItemFn, parse2};

    #[test]
    fn simple() {
        let item = quote! {
            fn square(x: u8) -> u8 {
                x * x
            }
        };
        let result = inner::precondition(
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
