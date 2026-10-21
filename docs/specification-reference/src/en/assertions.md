{{#rule contract.assertion}}

# Assertions

{{#syntax Assertion}}

## Semantics

`contract_assert!(P);` requires the Boolean condition `P` to hold at that point. It introduces a proof obligation using the function's preconditions, preceding assumptions, and the effects of the preceding program statements.

## Example

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(x < 16)]
pub fn bounded_square(x: u8) -> u8 {
    let y = x * x;
    contract_assert!(y <= 225);
    y
}
```

The assertion describes the local value `y` after multiplication.

## Implementation status

The macro expands the condition into a `Fn() -> bool + Copy` closure. The frontend extracts it as an assertion at its program location. An existing integration example and LLBC snapshot cover assertions.

`contract_assert!` is a specification annotation. It is removed in ordinary builds and does not provide the runtime check performed by Rust's `assert!` macro.
