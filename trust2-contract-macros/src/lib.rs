// functions tagged with `#[proc_macro_attribute]` must currently reside in the root of the crate

#[cfg(not(feature = "verify"))]
include!("lib-dummy.rs");

#[cfg(feature = "verify")]
include!("lib-impl.rs");
