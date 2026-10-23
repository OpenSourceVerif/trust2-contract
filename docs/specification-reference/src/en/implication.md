{{#rule contract.implies}}

# Implication

{{#syntax Implication}}

{{#rule contract.implies.intro}}

## Semantics

`implies(P, Q)` expresses logical implication. Both arguments have type `bool`. The proposition is false exactly when `P` is true and `Q` is false; its logical meaning is `!P || Q`.

This equivalence concerns truth values. The current call representation does not give `implies` the evaluation behavior of a short-circuit operator.

{{#rule contract.implies.example}}

## Example

```rust,ignore
use trust2_contract::prelude::*;

#[postcondition(|result| implies(x > 0, result >= x))]
pub fn identity(x: i32) -> i32 {
    x
}
```

The postcondition requires `result >= x` whenever `x > 0`.

The frontend can extract this implication expression and retain it inside quantified conditions.

{{#rule contract.implies.boundary}}

## Implementation boundaries <a id="contract.implies.boundary-title"></a>

- **Frontend `rust-llbc`**: recognizes and retains the `implies` marker in LLBC, including within quantifier bodies.
- **Backend `llbc-why3`**: does not yet translate this marker, so the example cannot complete verification.

### Remaining work

**Logical implication translation (backend).** The example's `implies(x > 0, result >= x)` needs to generate the logical condition that `result >= x` holds whenever `x > 0`. Markers within quantifier bodies need the same treatment.

Both arguments are currently evaluated, so `implies(i < a.len(), a[i] > 0)` cannot use its antecedent to guard indexing. Where short-circuit behavior is needed, write `i >= a.len() || a[i] > 0`; this remains subject to backend slice and index support. Whether `implies` itself should provide conditional evaluation remains open.
