use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn precondition(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn postcondition(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn invariant(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro]
pub fn contract_assert(_expr: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro]
pub fn contract_assume(_expr: TokenStream) -> TokenStream {
    TokenStream::new()
}
