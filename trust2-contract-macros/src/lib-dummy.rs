use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn precondition(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn postcondition(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
