{{#rule contract.invariant}}

# Type Invariants

{{#syntax TypeInvariant}}

{{#rule contract.invariant.intro}}

## Semantics

`#[invariant(P)]` associates a Boolean condition `P` with a struct, enum, or union. Within the condition, `self` refers to the value whose invariant is described. The condition expresses a property that valid instances are intended to satisfy.

Declaring an invariant and enforcing it at program boundaries are separate capabilities. The current implementation provides declaration and extraction; automatic enforcement is not yet implemented.

{{#rule contract.invariant.example}}

## Example

```rust,ignore
use trust2_contract::prelude::*;

#[invariant(self.start <= self.end)]
pub struct RefRange<'a, T: PartialOrd> {
    start: &'a T,
    end: &'a T,
}
```

The condition requires the start of the range to be no greater than its end. The type parameters and their bounds remain available in the invariant.

The frontend can extract this generic struct invariant and retain its association with the type.

An enum can describe a condition for each variant:

```rust,ignore
#[invariant(match self {
    Self::Empty => true,
    Self::Range { start, end } => start <= end,
})]
pub enum Range {
    Empty,
    Range { start: u32, end: u32 },
}
```

The frontend can extract this `match self` condition. Union conditions can also be extracted; reading a field still requires `unsafe` under Rust rules:

```rust,ignore
#[invariant(unsafe { self.x <= 100 })]
pub union Data {
    pub x: u32,
    pub y: u32,
}
```

These examples show how conditions are associated with types; they do not establish automatic backend enforcement.

{{#rule contract.invariant.boundary}}

## Implementation boundaries <a id="contract.invariant.boundary-title"></a>

- **Frontend `rust-llbc`**: extracts invariants on structs, enums, and unions and retains their association with the type. Multiple conditions on one type currently need to be combined with `&&`; repeated attributes conflict.
- **Backend `llbc-why3`**: does not yet use these conditions to establish invariant checks. Declaring an invariant does not yet guarantee valid objects.

### Remaining work

**From type conditions to checking obligations (backend; checking points remain open).** This type requires `x <= 100`, but constructing an invalid instance does not yet automatically generate the corresponding check:

```rust,ignore
#[invariant(self.x <= 100)]
pub struct B {
    pub x: u32,
}

pub fn invalid() -> B {
    B { x: 200 }
}
```

The frontend stores `x <= 100`; the generated WhyML does not yet use it. The condition needs to participate in verification of object creation, modification, and external use. Which boundaries require checks, whether modifications may temporarily violate the condition, and how it is restored when a borrow ends remain to be defined.
