{{#rule contract.constraints}}

# Implementation Notes

{{#rule contract.constraints.intro}}

## Reading implementation boundaries

Each construct page distinguishes **frontend `rust-llbc`** from **backend `llbc-why3`**. Here, the frontend includes macro expansion, Rust typing and borrowing checks, and extraction to LLBC. The backend translates specifications in LLBC to WhyML for verification.

Successful extraction does not establish backend support. Producing a WhyML file does not establish that it passes Why3 checking or that its obligations have been proved.

Examples show existing uses. “Remaining work” describes concrete scenarios, current obstacles, and the properties they are intended to express. Unsettled semantics are identified explicitly. These descriptions do not assign development stages, priorities, or an exhaustive support scope. See [Sources and Test Coverage](sources.md) for implementation sources and repository tests.

## Shared conventions

- **Rust typing and borrowing**: conditions return `bool` and use Rust's type system. Apart from type-invariant methods, specification closures require `Fn + Copy`; modifying captures or moving out non-`Copy` values may violate these requirements.
- **Ordinary builds**: specification annotations are removed. Successful ordinary compilation does not establish that specifications pass verification-mode checks.
- **Normal returns**: postconditions describe normal-return properties without also declaring termination or post-panic state.

## Name rewriting

Contract macros recursively rewrite `implies`, and postconditions also rewrite `old`. Quantifier macros do not perform these rewrites themselves; an enclosing contract macro handles them.

Current replacement is name-based and also affects ordinary same-named methods such as `value.old()`. The frontend needs to distinguish specification markers from ordinary Rust names. See [Lexical Rules](notations.md) for a concrete example.

## Rules still to be defined <a id="open-areas"></a>

Admissible function calls and restrictions on side effects are discussed under [Logic Expressions](expressions.md#contract.expression.boundary-title). The program boundaries at which type invariants hold are discussed under [Type Invariants](invariants.md#contract.invariant.boundary-title). These rules affect frontend and backend behavior; the concrete scenarios are retained without fixing a complete design in advance.
