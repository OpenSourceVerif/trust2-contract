{{#rule contract.quantifier}}

# Quantifiers

{{#syntax Quantifier QuantifierPredicate}}

{{#rule contract.quantifier.intro}}

## Semantics

`forall!(|x: T| P)` states that `P` holds for every value `x` of type `T`. `exists!(|x: T| P)` states that there is a value `x` of type `T` for which `P` holds. In both forms, `P` must have type `bool`.

The binding of `x` extends over the closure body. That body may refer to outer bindings and contain further quantifiers. Multiple quantified variables are expressed by nesting quantifiers, each with one parameter.

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

{{#rule contract.quantifier.boundary}}

## Implementation status <a id="contract.quantifier.boundary-title"></a>

The closure must satisfy `Fn(T) -> bool + Copy` and accept one quantified value. `|x: T, y: U|` is not a two-variable form of this interface. The frontend recursively extracts specification closures, including nested quantifiers.

The grammar uses an explicit parameter type. Omitting it relies on Rust type inference. There are no dedicated interval or set binders, or separate mathematical type domains.

`forall!` and `exists!` do not themselves rewrite `implies` or `old`. An enclosing contract macro performs that rewriting; only postconditions process `old`.

Both quantifiers have macro and extraction support. Existing integration examples and snapshots cover combinations using `forall`; they do not provide equivalent coverage for `exists`.
