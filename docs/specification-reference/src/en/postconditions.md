{{#rule contract.postcondition}}

# Postconditions

{{#syntax Postcondition ResultPredicate}}

## Semantics

`#[postcondition(|result| Q)]` requires the Boolean condition `Q` to hold on normal return, with `result` bound to the returned value. Verification of the function must establish this condition under its preconditions.

Multiple postconditions state simultaneous guarantees. They do not specify whether the function terminates or what happens on a panic.

## Return-value binding <a id="result-binding"></a>

The parameter name is arbitrary: `result` is not a keyword. Its type is the function's return type and may be written explicitly. A function without a declared return type returns `()`.

Function parameters and method receivers are referenced from the surrounding scope. A captured reference does not by itself denote a snapshot of the data it points to. The intended pre-state notation and its implementation limits are described under [Old-state Expressions](old.md).

## Example

```rust,ignore
use trust2_contract::prelude::*;

#[postcondition(|value: u32| value == x)]
pub fn identity(x: u32) -> u32 {
    x
}
```

Here `value` denotes the returned value, and `x` is the input used in the comparison.

Return-value conditions support frontend extraction and basic backend verification. For example, a maximum function can separately declare `result >= a` and `result >= b`, guaranteeing that its result is at least both inputs.

## Implementation boundaries

- **Frontend `rust-llbc`**: supports return-binding closures, references to input parameters, and multiple postconditions.
- **Backend `llbc-why3`**: supports basic scalar return-value conditions; relationships involving mutable references and old state remain incomplete.

### Remaining work

**Passing a function directly (frontend).** The following form passes Rust type checking but fails during specification extraction:

```rust,ignore
fn positive(value: u32) -> bool {
    value > 0
}

#[postcondition(positive)]
pub fn one() -> u32 {
    1
}
```

Writing the same condition as `#[postcondition(|value| positive(value))]` allows extraction. A directly supplied function still needs to be associated with the return-value condition, or rejected with a clear diagnostic at the entry point. Verification limits on the call itself are described under [Logic Expressions](expressions.md#contract.expression.boundary-title).

**Before/after relationships.** Stating that `*p` is one greater on return than on entry still requires [`old`](old.md#contract.old.boundary-title) and a complete state representation for mutable references. Capturing `p` alone does not establish an entry-value snapshot.
