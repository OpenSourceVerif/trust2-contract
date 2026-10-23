{{#rule contract.precondition}}

# Preconditions

{{#syntax Precondition}}

## Semantics

`#[precondition(P)]` requires the Boolean condition `P` to hold on entry to the function. At a call, the caller must establish `P`; when verifying the function body, `P` is an assumption.

The condition may refer to the function's parameters, a method receiver, and other names visible at the definition. It does not bind a return value. Multiple precondition attributes require all of their conditions to hold.

## Example

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(divisor != 0)]
pub fn quotient(value: u32, divisor: u32) -> u32 {
    value / divisor
}
```

The precondition excludes division by zero. A call with `divisor == 0` fails the stated requirement.

Basic parameter conditions support frontend extraction. Existing backend examples include constraining a square calculation with `x < 16` and unsigned subtraction with `a >= b`.

## Implementation boundaries

- **Frontend `rust-llbc`**: supports basic parameter conditions, method receivers, and multiple preconditions.
- **Backend `llbc-why3`**: supports translation and verification of basic scalar conditions.

See [Function Contracts](function-contracts.md#contract.function.boundary-title) for bodyless declarations and [Logic Expressions](expressions.md#contract.expression.boundary-title) for conditions involving slices and other expressions.
