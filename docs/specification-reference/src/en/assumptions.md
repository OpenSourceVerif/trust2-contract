{{#rule contract.assumption}}

# Assumptions

{{#syntax Assumption}}

## Semantics

`contract_assume!(P);` introduces the Boolean condition `P` as a hypothesis at that point. It does not introduce an obligation to prove `P`.

Conclusions obtained using the assumption are conditional on it. An inconsistent assumption makes subsequent proof obligations vacuous.

## Example

```rust,ignore
use trust2_contract::prelude::*;

pub fn assumed_bound(x: u8) {
    contract_assume!(x <= 10);
    contract_assert!(x < 11);
}
```

The assertion follows under the assumed bound. The annotation neither checks nor establishes that bound for the caller.

## Implementation status

The macro and internal interface represent the condition as a `Fn() -> bool + Copy` closure. A dedicated extraction branch distinguishes assumptions from assertions. The existing frontend integration examples do not cover assumptions.

Assumptions are removed in ordinary builds. They do not constrain a runtime value or reject an input.
