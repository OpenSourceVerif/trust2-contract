{{#rule contract.sources}}

# Sources and Test Coverage

{{#rule contract.sources.intro}}

The following sources describe frontend extraction, backend translation, and existing coverage.

## Frontend rust-llbc

- [Contract macros](../../../../trust2-contract-macros/src/impl.rs) and [identifier rewriting](../../../../trust2-contract-macros/src/impl/parse.rs): parsing and expansion.
- [Internal interfaces](../../../../trust2-contract/src/internal.rs): argument types and closure constraints.
- [Function-body extraction](../../../../charon/charon/src/transform/collect_specs/unstructured_body.rs) and [type-specification extraction](../../../../charon/charon/src/transform/collect_specs/non_body.rs): associating specifications with programs.
- [Marker recognition](../../../../charon/charon/src/bin/charon-driver/translate/translate_functions.rs): implication and old-state markers.
- [Frontend integration examples](../../../../charon/charon/tests/cargo/trust2-contract-sample/src/lib.rs) and [LLBC snapshots](../../../../charon/charon/tests/cargo/trust2-contract-sample.out): existing combined cases.

## Backend llbc-why3

- [Function-contract translation](../../../../cargo-verify/src/llbc_to_whyml/ctx/decl.rs) and [specification-closure translation](../../../../cargo-verify/src/llbc_to_whyml/ctx/spec.rs): preconditions, postconditions, and captured values.
- [Statement translation](../../../../cargo-verify/src/llbc_to_whyml/ctx/stmt_expr.rs) and [logical-term translation](../../../../cargo-verify/src/llbc_to_whyml/ctx/stmt_term.rs): assertion, assumption, and quantifier branches.
- [Builtin-function translation](../../../../cargo-verify/src/llbc_to_whyml/ctx/func_ref.rs): unimplemented `SpecImplies` and `SpecOld` branches.
- [Type translation](../../../../cargo-verify/src/llbc_to_whyml/ctx/ty.rs) and [type-declaration translation](../../../../cargo-verify/src/llbc_to_whyml/ctx/type_decl.rs): supported types; automatic type-invariant enforcement is not connected.
- [End-to-end examples](../../../../cargo-verify/tests/end-to-end/simple/src/lib.rs) and [saved results](../../../../cargo-verify/tests/end-to-end/simple/result.txt): basic contract verification for `max`, `strict_sub`, and `square`.

## Test coverage

Frontend integration cases include preconditions, postconditions, assertions, a generic struct invariant, and combinations of universal quantification, implication, and container access. The `to_sorted` body is a placeholder returning an empty vector; it illustrates extraction rather than a verified sorting algorithm.

`old` cases cover a mutable scalar, a slice inside a quantifier, and a separate crate of rejected uses. A further crate covers `==>`, `each!`, `exists!`, and `contract_assume!`, and is extracted with the same flags as `cargo verify`.

Saved backend end-to-end results cover basic preconditions/postconditions for `max`, `strict_sub`, and `square`. Assertions, assumptions, and quantifiers have translation implementations but are not included in these end-to-end cases.
