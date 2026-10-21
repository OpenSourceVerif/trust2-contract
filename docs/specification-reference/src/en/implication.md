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

{{#rule contract.implies.boundary}}

## Implementation status <a id="contract.implies.boundary-title"></a>

Contract macros rewrite `implies` to an internal function, and the frontend recognizes and retains a `SpecImplies` marker. Existing combined examples cover this representation.

Both arguments' computations enter the specification body. In particular, `implies(i < a.len(), a[i] > 0)` does not automatically guard the index access with the antecedent.

When an access must be conditional, use Rust short-circuit operators. For example, `i >= a.len() || a[i] > 0` avoids evaluating the index expression when `i` is out of bounds.
