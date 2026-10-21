{{#rule contract.sources}}

# Sources and Test Coverage

{{#rule contract.sources.intro}}

The following sources describe macro expansion, frontend extraction, and existing test coverage.

- [Contract macros](../../../../trust2-contract-macros/src/impl.rs) and [identifier rewriting](../../../../trust2-contract-macros/src/impl/parse.rs): parsing and expansion.
- [Internal interfaces](../../../../trust2-contract/src/internal.rs): argument types and closure constraints.
- [Function-body extraction](../../../../charon/charon/src/transform/collect_specs/unstructured_body.rs) and [type-specification extraction](../../../../charon/charon/src/transform/collect_specs/non_body.rs): associating specifications with programs.
- [Marker recognition](../../../../charon/charon/src/bin/charon-driver/translate/translate_functions.rs): implication and old-state markers.
- [Frontend integration examples](../../../../charon/charon/tests/cargo/trust2-contract-sample/src/lib.rs) and [LLBC snapshots](../../../../charon/charon/tests/cargo/trust2-contract-sample.out): existing combined cases.
