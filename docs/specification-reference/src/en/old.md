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

## Implementation boundaries <a id="contract.old.boundary-title"></a>

- **Frontend `rust-llbc`**: recognizes the `old` marker but does not yet fully represent entry values; its current interface accepts only mutable references.
- **Backend `llbc-why3`**: old-state translation is not implemented.

### Remaining work

**Entry values of mutable parameters (frontend and backend).** The following example aims to describe an increment but cannot currently be used:

```rust,ignore
#[precondition(*p < u32::MAX)]
#[postcondition(|_| *p == *old(p) + 1)]
fn increment(p: &mut u32) {
    *p += 1;
}
```

In verification mode, this form is blocked by the closure's `Copy` requirement. The frontend needs to let postconditions refer to entry values and distinguish them from return-state values. The backend needs to use that representation to establish the relationship. The intended property is that `*p` is one greater than its entry value; the interface may change with the state representation.

The scope of old-state support for fields, compound values, and aliased data remains to be defined.
