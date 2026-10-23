{{#rule contract.statement}}

# Statement Annotations

Statement annotations express conditions at a particular point in a function body.

{{#syntax ContractStatement}}

{{#rule contract.statement.intro}}

## Semantics

An [assertion](assertions.md) states a condition to establish at its location. An [assumption](assumptions.md) supplies a condition for reasoning from that location. Both may refer to variables in scope and describe the state at the annotation.

{{#rule contract.statement.example}}

## Example

```rust,ignore
use trust2_contract::prelude::*;

pub fn check(x: u8) {
    contract_assume!(x < 16);
    contract_assert!(x <= 15);
}
```

The assertion follows from the preceding assumption. This does not prove that `x < 16` holds for every call to `check`.

{{#rule contract.statement.boundary}}

## Implementation boundaries <a id="contract.statement.boundary-title"></a>

- **Frontend `rust-llbc`**: supports assertions and assumptions, retaining their conditions, kinds, and positions.
- **Backend `llbc-why3`**: translates them to WhyML `assert` and `assume`, respectively.

See [Assertions](assertions.md) and [Assumptions](assumptions.md) for specific uses and limitations.
