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

The frontend can extract local assertions of this form. It can also retain ordinary calls in conditions, such as `contract_assert!(decuple(9) == 90)`.

## Implementation boundaries

- **Frontend `rust-llbc`**: extracts assertion conditions and retains their positions in function bodies, including within branches.
- **Backend `llbc-why3`**: translates them to WhyML `assert`; further verification depends on the expressions and program operations involved.

Shared limitations are described under [Logic Expressions](expressions.md#contract.expression.boundary-title). Ordinary builds remove contract assertions; runtime checks use Rust's `assert!`.
