{{#rule contract.implies}}

# Implication

{{#syntax Implication}}

{{#rule contract.implies.intro}}

## Semantics

`P ==> Q` expresses logical implication: it is false exactly when `P` is true and `Q` is false. `P` and `Q` have type `bool`.

`==>` binds weaker than every other operator and associates to the right, so `a && b ==> c == d` means `(a && b) ==> (c == d)` and `a ==> b ==> c` means `a ==> (b ==> c)`. Parentheses are needed to use the result as an operand, as in `(a ==> b) && c`. A condition after `if` or `match` must also be parenthesized: `if (a ==> b) { ... }`.

The contract macros rewrite `P ==> Q` to `!(P) || (Q)`, so `Q` is evaluated only when `P` holds. A condition on an index can therefore guard the access that depends on it, as in `i < v.len() ==> v[i] > 0`.

`implies(P, Q)` has the same truth table but is an ordinary call: both arguments are evaluated. Prefer `==>`.

{{#rule contract.implies.example}}

## Example

```rust,ignore
use trust2_contract::prelude::*;

#[postcondition(|result| x > 0 ==> result >= x)]
pub fn identity(x: i32) -> i32 {
    x
}

#[precondition(forall!(|i: usize| i < v.len() ==> v[i] > 0))]
pub fn all_positive(v: &[u8]) {}
```

The first postcondition requires `result >= x` whenever `x > 0`. The second condition does not read `v[i]` for an index outside the slice.

{{#rule contract.implies.boundary}}

## Implementation boundaries <a id="contract.implies.boundary-title"></a>

- **Frontend `rust-llbc`**: `==>` is rewritten by the contract macros and reaches LLBC as a branch, without a dedicated marker. `implies` is recognized and retained as a marker in LLBC.
- **Backend `llbc-why3`**: `==>` translates to a WhyML conditional such as `if x > 0 then result >= x else true`. The `implies` marker is not translated. Conditions on slices are subject to the [backend limits on slices](expressions.md#contract.expression.boundary-title).

### Remaining work

**The `implies` marker (backend).** `implies(x > 0, result >= x)` needs a translation, or `implies` can be removed in favor of `==>`.
