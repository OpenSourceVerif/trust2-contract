use proc_macro::TokenStream;

mod r#impl;

#[proc_macro_attribute]
pub fn precondition(attr: TokenStream, item: TokenStream) -> TokenStream {
    r#impl::precondition(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn postcondition(attr: TokenStream, item: TokenStream) -> TokenStream {
    r#impl::postcondition(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn invariant(attr: TokenStream, item: TokenStream) -> TokenStream {
    r#impl::invariant(attr.into(), item.into()).into()
}

#[proc_macro]
pub fn contract_assert(expr: TokenStream) -> TokenStream {
    r#impl::contract_assert(expr.into()).into()
}

#[proc_macro]
pub fn contract_assume(expr: TokenStream) -> TokenStream {
    r#impl::contract_assume(expr.into()).into()
}
