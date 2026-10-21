{{#rule contract.function}}

# Function Contracts

A function contract specifies conditions required when a function is called and conditions guaranteed when it returns normally.

{{#syntax FunctionContract}}

{{#rule contract.function.intro}}

## Semantics

<a id="grammar-Precondition"></a>[Preconditions](preconditions.md) describe the caller's obligations and the assumptions under which the function body is verified. <a id="grammar-Postcondition"></a>[Postconditions](postconditions.md) describe the function's obligations on return.

Attributes precede the function definition. Preconditions and postconditions may be repeated and interleaved. All stated preconditions are required; all stated postconditions are guarantees to establish. A postcondition alone does not assert termination.

<a id="grammar-ResultPredicate"></a><a id="result-binding"></a>A postcondition binds the returned value with a closure parameter. See [Return-value binding](postconditions.md#result-binding). References to an earlier state use [Old-state Expressions](old.md), whose current interface is restricted.

{{#rule contract.function.example}}

## Example

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(x < 16)]
#[postcondition(|result| result >= x)]
pub fn square(x: u8) -> u8 {
    x * x
}
```

The precondition restricts `x` to values whose square fits in `u8`. The postcondition requires the returned value to be at least `x`.

{{#rule contract.function.boundary}}

## Implementation status <a id="contract.function.boundary-title"></a>

Contracts can be expanded and extracted for ordinary functions with bodies, nested functions, inherent methods, associated functions, trait default methods, and methods in impl blocks. Existing integration examples cover preconditions and postconditions.

Signature-only trait methods and external function declarations are not supported. There is no general mechanism for trait-contract inheritance or refinement. Verification-mode macro expansion does not support `const fn`; the frontend cannot yet extract async functions.

The current contract interface has no clauses for modification sets, termination measures, or exceptional exits. Per-clause typing and extraction constraints are described on the following pages.
