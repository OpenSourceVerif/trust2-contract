{{#rule contract.constraints}}

# Implementation Notes

These notes describe shared properties of the frontend under development. Construct-specific status is documented on the corresponding reference pages.

{{#rule contract.constraints.intro}}

## Representation

Specifications must satisfy the surface syntax, Rust typing and borrowing, and the structure expected by extraction. Conditions return `bool`. Apart from the type-invariant method, specification closures are constrained by `Fn + Copy`.

Ordinary builds remove specifications. Successful compilation in that mode is not evidence that the specification passes verification-mode checks.

## Name rewriting

Within tokens processed by a contract macro, `implies` and, in postconditions, `old` are rewritten recursively. Using those names for unrelated bindings, fields, or methods can therefore interfere with rewriting. General call syntax does not bypass the dedicated interfaces' constraints.

## Open areas

The current specification interface does not provide loop invariants, loop or recursion termination measures, read/write footprints or modification sets, permission or borrowing contracts, contracts for exceptional exits, module contracts, or supplementary specifications for external interfaces.

See [Sources and Test Coverage](sources.md) for the implementation and examples behind these descriptions, and [Lexical Rules](notations.md#rust-syntax) for the Rust syntax used in grammar blocks.
