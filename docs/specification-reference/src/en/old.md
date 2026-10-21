{{#rule contract.old}}

# Old-state Expressions

{{#syntax OldExpression}}

{{#rule contract.old.intro}}

## Intended semantics

An old-state expression in a postcondition refers to the function's pre-state: the state on entry to the function. Its purpose is to relate a returned state to an earlier one.

The current `old` construct provides a marker for this purpose, with the restricted interface described below. It does not yet provide a general snapshot operation on Rust expressions.

{{#rule contract.old.example}}

## Syntax example

```rust,ignore
*old(p)
```

This fragment combines an old-state marker with dereferencing. The argument `p` must match the mutable-reference interface. Whether the fragment is admissible in a postcondition also depends on the surrounding closure's captures and borrowing.

{{#rule contract.old.boundary}}

## Implementation status <a id="contract.old.boundary-title"></a>

Postcondition macros rewrite the name `old`; the frontend recognizes and retains a `SpecOld` marker. Other contract macros do not perform this rewriting.

The internal signature is:

```rust,ignore
pub fn old<T>(x: &mut T) -> &mut T
```

The postcondition must still satisfy `Fn + Copy`. Mutable captures are restricted by these requirements. For an integer `x`, `old(x + 1)` does not match the interface even though its surface structure matches `OldExpression`.

Name-replacement tests cover rewriting, and the frontend has marker recognition. These stages do not implement snapshots of arbitrary values or reachable heaps, or a mechanism for preserving unchanged memory regions. Complete old-state specifications are not covered by the listed integration examples.
