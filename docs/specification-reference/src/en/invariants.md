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

{{#rule contract.invariant.boundary}}

## Implementation status <a id="contract.invariant.boundary-title"></a>

The macro preserves generic parameters and bounds and generates a `TypeInvariant` implementation with an `invariant(&self) -> bool` method. The frontend extracts the condition and associates it with the type. The generic `RefRange` example has an existing integration snapshot.

The implementation does not automatically insert obligations at construction, field updates, or the end of a borrow to establish or restore the invariant. Declaring the attribute alone does not ensure that every instance satisfies the condition.

Type aliases cannot carry this attribute. Combine requirements in a single condition, for example with `&&`; repeated attributes generate conflicting trait implementations. Macro branches accept enums and unions, but the listed integration examples do not cover them. Union-field access remains subject to Rust's safety rules.
