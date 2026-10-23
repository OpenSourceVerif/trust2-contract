{{#rule contract.quantifier}}

# Quantifiers

{{#syntax Quantifier QuantifierPredicate}}

{{#rule contract.quantifier.intro}}

## Semantics

`forall!(|x: T| P)` states that `P` holds for every value `x` of type `T`. `exists!(|x: T| P)` states that there is a value `x` of type `T` for which `P` holds. In both forms, `P` must have type `bool`.

The binding of `x` extends over the closure body. That body may refer to outer bindings and contain further quantifiers. Multiple quantified variables can be expressed by nesting quantifiers or destructuring a single tuple parameter. Each quantifier accepts one parameter; a two-parameter closure such as `|x: u8, y: u8|` does not match the current interface.

The domain is a Rust type. For example, quantifying over `u8` concerns its finite range of values, not unbounded mathematical integers. A restriction on the domain is expressed in the body: universal quantification uses a conditional property, while existential quantification requires both the restriction and the property to hold.

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

This form also supports extraction and WhyML generation. Capturing other outer variables and combining nested quantifiers are discussed in the implementation boundaries below.

{{#rule contract.quantifier.boundary}}

## Implementation boundaries <a id="contract.quantifier.boundary-title"></a>

- **Frontend `rust-llbc`**: has extraction for both quantifiers and nested quantifiers, and retains tuple parameters; passing captured outer variables still has problems.
- **Backend `llbc-why3`**: has universal and existential quantifier translation, but producing a file does not establish that bindings and conditions were handled correctly.

### Remaining work

**Passing outer variables (frontend extraction and backend use).** This condition expresses equality between the returned value and the input, but does not currently translate correctly:

```rust,ignore
#[postcondition(|r| exists!(|x: u8| x == r && x == n))]
pub fn identity(n: u8) -> u8 {
    n
}
```

The example produces LLBC, but the subsequent WhyML refers to an undefined variable. Problems also occur when quantifiers reference function parameters or multiple quantifier levels capture outer variables. Extraction and translation need to pass each level's captures correctly, keeping `n` associated with the input, `r` with the returned value, and `x` with the local binding.

Quantification over ranges and slices also depends on [the corresponding expressions](expressions.md#contract.expression.boundary-title). For example, stating that every valid index refers to a positive element requires quantifiers, lengths, and indexing to work together.
