mod r#impl;

// functions tagged with `#[proc_macro_attribute]` must currently reside in the root of the crate

#[cfg(not(feature = "verify"))]
include!("lib-dummy.rs");

#[cfg(feature = "verify")]
include!("lib-impl.rs");

#[proc_macro]
pub fn forall(expr: TokenStream) -> TokenStream {
    r#impl::forall(expr.into()).into()
}

#[proc_macro]
pub fn exists(expr: TokenStream) -> TokenStream {
    r#impl::exists(expr.into()).into()
}
