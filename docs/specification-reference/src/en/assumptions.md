{{#rule contract.assumption}}

# Assumptions

{{#syntax Assumption}}

## Semantics

`contract_assume!(P);` introduces the Boolean condition `P` as a hypothesis at that point. It does not introduce an obligation to prove `P`.

Conclusions obtained using the assumption are conditional on it. An inconsistent assumption makes subsequent proof obligations vacuous.

## Example

```rust,ignore
use trust2_contract::prelude::*;

pub fn assumed_bound(x: u8, flag: bool) {
    if flag {
        contract_assume!(x <= 10);
        contract_assert!(x < 11);
    } else {
        contract_assert!(x <= 255);
    }
}
```

On the branch where `flag` is true, the assumed bound implies the following assertion; the other branch does not use that assumption. Frontend extraction and WhyML translation retain this branch structure. The annotation neither checks nor establishes the bound for the caller.

## Implementation boundaries

- **Frontend `rust-llbc`**: extracts assumptions, distinguishes them from assertions, and retains their enclosing branches.
- **Backend `llbc-why3`**: translates them to WhyML `assume`; further verification depends on the expressions and program operations involved.

Shared limitations are described under [Logic Expressions](expressions.md#contract.expression.boundary-title). Assumptions create no obligation to prove themselves, and ordinary builds do not use them to filter inputs.
