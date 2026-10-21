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

## Implementation status

The macro parses the condition as an expression and represents it as a `Fn() -> bool + Copy` closure. Rust typing, capture, and borrowing constraints apply. The frontend associates the extracted condition with the function as a precondition.

Preconditions have existing integration examples and LLBC snapshots. The supported function forms and shared limitations are listed under [Function Contracts](function-contracts.md#contract.function.boundary-title).
