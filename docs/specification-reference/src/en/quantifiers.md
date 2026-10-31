{{#rule contract.quantifier}}

# Quantifiers

{{#syntax Quantifier QuantifierPredicate EachDomain}}

{{#rule contract.quantifier.intro}}

## Semantics

`forall!(|x: T| P)` states that `P` holds for every value `x` of type `T`. `exists!(|x: T| P)` states that there is a value `x` of type `T` for which `P` holds. In both forms, `P` must have type `bool`.

The binding of `x` extends over the closure body. That body may refer to outer bindings and contain further quantifiers. Multiple quantified variables can be expressed by nesting quantifiers or destructuring a single tuple parameter. Each quantifier accepts one parameter; a two-parameter closure such as `|x: u8, y: u8|` does not match the current interface.

The domain is a Rust type. For example, quantifying over `u8` concerns its finite range of values, not unbounded mathematical integers. A restriction on the domain is expressed in the body: universal quantification uses a conditional property, while existential quantification requires both the restriction and the property to hold.

`each!` is a universal quantifier with an explicit domain:

- `each!(i: T; G; P)` states that `P` holds for every `i` of type `T` satisfying `G`.
- `each!(i: T in a..b; P)` is the same with `G` being `a <= i && i < b`. `a..=b` includes `b`.

`P` is evaluated only for values in the domain, so `each!(i: usize in 0..v.len(); v[i] > 0)` does not read outside the slice. Both bounds of a range are required. Bounds need not be constant.

{{#rule contract.quantifier.example}}

## Examples

```rust,ignore
use trust2_contract::prelude::*;

#[postcondition(|result| forall!(|i: u8| i <= result))]
pub fn largest_u8() -> u8 {
    u8::MAX
}

#[postcondition(|result| exists!(|i: u8| i == result))]
pub fn zero() -> u8 {
    0
}
```

The first postcondition gives an upper bound for all `u8` values. The second states that the returned value has a witness in the same type.

Both examples retain their quantifiers and return-value bindings in LLBC and generate the corresponding WhyML quantified conditions.

A tuple parameter can introduce two variables at once. This condition states that any two `u8` values are equal or the returned value is zero:

```rust,ignore
#[postcondition(|r| forall!(|(x, y): (u8, u8)| x == y || r == 0))]
pub fn zero_for_pairs() -> u8 {
    0
}
```

This form also supports extraction and WhyML generation.

`each!` restricts the domain directly. Each of these states that every `i` below the bound is below the result:

```rust,ignore
#[postcondition(|r| each!(i: u8; i < n; i < r))]
pub fn with_guard(n: u8) -> u8 {
    n
}

#[postcondition(|r| each!(i: u8 in 0..n; i < r))]
pub fn with_range(n: u8) -> u8 {
    n
}
```

{{#rule contract.quantifier.boundary}}

## Implementation boundaries <a id="contract.quantifier.boundary-title"></a>

- **Frontend `rust-llbc`**: has extraction for all three quantifiers and nested quantifiers, and retains tuple parameters and captured outer variables. `each!` is expanded by its macro to `forall!` and reaches LLBC as a universal quantifier with a branch on the domain.
- **Backend `llbc-why3`**: has universal and existential quantifier translation, but producing a file does not establish that bindings and conditions were handled correctly.

### Remaining work

**Quantification over ranges and slices (backend).** Conditions on slices depend on [the corresponding expressions](expressions.md#contract.expression.boundary-title). For example, stating that every valid index refers to a positive element requires quantifiers, lengths, and indexing to work together.
